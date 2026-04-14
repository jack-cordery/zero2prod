use actix_web::{
    HttpResponse, ResponseError,
    body::BoxBody,
    web::{self, ReqData},
};
use actix_web_flash_messages::FlashMessage;
use anyhow::{Context, anyhow};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    authentication::UserId,
    idempotency::{
        IndempotencyKey, NextAction, PgTransaction, get_saved_response, initialise_response,
        save_response,
    },
    issue_delivery_worker::Retries,
    routes::error_chain_fmt,
    utils::see_other,
};

#[derive(serde::Deserialize)]
pub struct PublishForm {
    title: String,
    text: String,
    html: String,
    idempotency_key: String,
}

impl PublishForm {
    pub fn validate_user_inputs(&self) -> Result<(), PublishError> {
        if self.title.is_empty() | self.text.is_empty() | self.html.is_empty() {
            Err(PublishError::InvalidForm(anyhow!("a field was empty")))
        } else {
            Ok(())
        }
    }
}

#[derive(thiserror::Error)]
pub enum PublishError {
    #[error("Invalid form details provided.")]
    InvalidForm(#[source] anyhow::Error),
    #[error("An unexpected error occurred. Please try again.")]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for PublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for PublishError {
    fn error_response(&self) -> HttpResponse<BoxBody> {
        FlashMessage::error(self.to_string()).send();
        see_other("/admin/newsletter")
    }
}

#[tracing::instrument(name = "Enqueue emails", skip(tx, max_retries))]
pub async fn enqueue_emails(
    newsletter_id: Uuid,
    max_retries: &Retries,
    tx: &mut PgTransaction,
) -> Result<(), anyhow::Error> {
    sqlx::query!(
        r#"
            INSERT INTO email_processing_tasks (subscriber_id, newsletter_id, retries, execute_after)
            SELECT id, $1, $2, now()
            FROM subscriptions
            WHERE status='confirmed';
    "#,
        newsletter_id,max_retries.to_db()
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[tracing::instrument(name = "Persist newsletter issue", skip(tx))]
pub async fn persist_newsletter_issue(
    user_id: &UserId,
    title: String,
    text: String,
    html: String,
    tx: &mut PgTransaction,
) -> Result<Uuid, anyhow::Error> {
    let id = Uuid::new_v4();
    sqlx::query!(
        r#"
    INSERT INTO newsletters(id, user_id, title, content_html, content_text)
    VALUES(
    $1,
    $2,
    $3,
    $4,
    $5
    );
    "#,
        id,
        **user_id,
        title,
        text,
        html
    )
    .execute(&mut **tx)
    .await?;
    Ok(id)
}

#[tracing::instrument(name = "Publish newsletter", skip(form,pool, max_retries), fields(newsletter_title=%form.title))]
pub async fn publish_newsletter(
    form: web::Form<PublishForm>,
    pool: web::Data<PgPool>,
    user_id: ReqData<UserId>,
    max_retries: web::Data<Retries>,
) -> Result<HttpResponse, PublishError> {
    let mut tx = pool.begin().await.context("Unable to begin transaction")?;
    form.validate_user_inputs()?;
    let user_id = user_id.into_inner();
    let max_retries: &Retries = max_retries.get_ref();

    let PublishForm {
        title,
        text,
        html,
        idempotency_key,
    } = form.0;

    let idempotency_key: IndempotencyKey = idempotency_key
        .to_owned()
        .try_into()
        .map_err(PublishError::UnexpectedError)?;

    match initialise_response(&user_id, &idempotency_key, &mut tx).await? {
        NextAction::RetrieveResponse => {
            FlashMessage::info("Newsletter published").send();
            let response = get_saved_response(&user_id, &idempotency_key, &mut tx)
                .await?
                .context("expected a fully formed saved response")?;
            Ok(response)
        }
        NextAction::ProcessEmails => {
            let newsletter_id =
                persist_newsletter_issue(&user_id, title, text, html, &mut tx).await?;
            match enqueue_emails(newsletter_id, max_retries, &mut tx).await {
                Ok(_) => {
                    FlashMessage::info("Newsletter published").send();
                    let response = see_other("/admin/dashboard");
                    let response =
                        save_response(&user_id, &idempotency_key, response, &mut tx).await?;
                    tx.commit().await.context(
                        "Failed to commit transaction to complete publishing newsletter",
                    )?;
                    Ok(response)
                }
                Err(e) => {
                    FlashMessage::info(e.to_string()).send();
                    let response = see_other("/admin/newsletter");
                    Ok(response)
                }
            }
        }
    }
}
