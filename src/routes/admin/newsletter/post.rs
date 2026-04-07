use actix_web::{
    HttpResponse, ResponseError,
    body::BoxBody,
    web::{self, ReqData},
};
use actix_web_flash_messages::FlashMessage;
use anyhow::{Context, anyhow};
use sqlx::PgPool;

use crate::{
    authentication::UserId,
    domain::{SubscriberEmail, SubscriberStatus},
    email_client::EmailClient,
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

pub struct ConfirmedSubscriber {
    email: SubscriberEmail,
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

pub async fn try_processing(
    user_id: &UserId,
    publish_form: &PublishForm,
    email_client: &EmailClient,
    tx: &mut PgTransaction,
) -> Result<HttpResponse, anyhow::Error> {
    let idempotency_key: IndempotencyKey = publish_form
        .idempotency_key
        .to_owned()
        .try_into()
        .map_err(PublishError::UnexpectedError)?;

    let initialised_response = initialise_response(user_id, &idempotency_key, tx).await?;

    match initialised_response {
        NextAction::ProcessEmails => {
            let confirmed_subscriptions = get_confirmed_subscriptions(tx).await?;
            for sub in confirmed_subscriptions {
                match sub {
                    Ok(s) => {
                        email_client
                            .send_email(
                                &s.email,
                                &publish_form.title,
                                &publish_form.text,
                                &publish_form.html,
                            )
                            .await
                            .with_context(|| format!("Failed to send email to {}", s.email))?;
                    }
                    Err(e) => {
                        tracing::warn!("Confirmed subscriber is using invalide email {}", e)
                    }
                }
            }
            let http_response = see_other("/admin/dashboard");
            let http_response = save_response(user_id, &idempotency_key, http_response, tx).await?;
            Ok(http_response)
        }
        NextAction::RetrieveResponse => {
            let saved_response = get_saved_response(user_id, &idempotency_key, tx)
                .await?
                .context("Expected a saved response")?;
            Ok(saved_response)
        }
    }
}

#[tracing::instrument(name = "Publish newsletter", skip(form,pool, email_client), fields(newsletter_title=%form.title))]
pub async fn publish_newsletter(
    form: web::Form<PublishForm>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    user_id: ReqData<UserId>,
) -> Result<HttpResponse, PublishError> {
    let mut tx = pool.begin().await.context("Unable to begin transaction")?;
    form.validate_user_inputs()?;
    let user_id = user_id.into_inner();

    let response = try_processing(&user_id, &form.0, &email_client, &mut tx).await?;
    tx.commit()
        .await
        .context("Failed to commit transaction to complete publishing newsletter")?;
    FlashMessage::info("Newsletter published").send();
    Ok(response)
}

#[tracing::instrument(name = "Get confirmed subscriptions", skip(tx))]
pub async fn get_confirmed_subscriptions(
    tx: &mut PgTransaction,
) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
    let confirmed_status: String = SubscriberStatus::Confirmed.into();
    let confirmed_rows = sqlx::query!(
        r#"SELECT email FROM subscriptions WHERE status = $1"#,
        confirmed_status,
    )
    .fetch_all(&mut **tx)
    .await?;
    let confirmed_subscriptions = confirmed_rows
        .into_iter()
        .map(|r| {
            let sub_email = SubscriberEmail::parse(r.email);
            match sub_email {
                Ok(s) => Ok(ConfirmedSubscriber { email: s }),
                Err(error) => Err(anyhow::anyhow!(error)),
            }
        })
        .collect();
    Ok(confirmed_subscriptions)
}
