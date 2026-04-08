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

// TODO: So we have the problem at the moment that retry causes partial states
// i.e. we can have that n/N_subscribers emails be sent out where 0 < n < N_subscribers
// and we need a way to recover. We can backtrack i.e. send out an apology or delete an email
// and so we will use forward recovery. To do so we will need to jiggle thing around a bit.
// - [X] Create a table to persist newsletter issue (user_id, newsletter_id, title, text, html)
// - [X] Create a Queue table this will store our tasks for a worker to complete. (sub_id,
// idempotent_key/newsletter_issue?)
// - [] Change publish newsletter end point to just requesting it to be published.
//   - It will respond Ok to user on successful request
//   - It will enque all subs into the queue table
//   - It will return saved response if there is one, and will deal with concurrency as is.
// - [] Create a background worker using spawn + select in startup.rs
//   - It will constantly pick with SELECT FOR UPDATE SKIP LOCKED the first available in the queue
//     and try to send the email to the user. For transient errors it will not delete the row and
//     skip it. For non-transient errors it will delete it. On successful email it will delete the
//     task
// - [] Create a background task to clean up expired idempotent keys and associated tasks
//

#[tracing::instrument(name = "Enqueue emails", skip(tx))]
pub async fn enqueue_emails(
    newsletter_id: Uuid,
    tx: &mut PgTransaction,
) -> Result<(), anyhow::Error> {
    sqlx::query!(
        r#"
            INSERT INTO email_processing_tasks (subscriber_id, newsletter_id)
            SELECT id, $1
            FROM subscriptions
            WHERE status='confirmed';
    "#,
        newsletter_id
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

#[tracing::instrument(name = "Publish newsletter", skip(form,pool), fields(newsletter_title=%form.title))]
pub async fn publish_newsletter(
    form: web::Form<PublishForm>,
    pool: web::Data<PgPool>,
    user_id: ReqData<UserId>,
) -> Result<HttpResponse, PublishError> {
    let mut tx = pool.begin().await.context("Unable to begin transaction")?;
    form.validate_user_inputs()?;
    let user_id = user_id.into_inner();

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
            let response = get_saved_response(&user_id, &idempotency_key, &mut tx)
                .await?
                .context("expected a fully formed saved response")?;
            Ok(response)
        }
        NextAction::ProcessEmails => {
            let newsletter_id =
                persist_newsletter_issue(&user_id, title, text, html, &mut tx).await?;
            match enqueue_emails(newsletter_id, &mut tx).await {
                Ok(_) => {
                    //
                    tx.commit().await.context(
                        "Failed to commit transaction to complete publishing newsletter",
                    )?;
                    FlashMessage::info("Newsletter published").send();
                    let response = see_other("/admin/dashboard");
                    Ok(response)
                }
                Err(e) => {
                    FlashMessage::info(e.to_string()).send();
                    let response = see_other("/admin/newsletter");
                    let response =
                        save_response(&user_id, &idempotency_key, response, &mut tx).await?;
                    Ok(response)
                }
            } // enqueue emails and all that jazz
        }
    }
}
