use actix_web::{
    HttpResponse, ResponseError,
    body::BoxBody,
    http::header::LOCATION,
    web::{self, ReqData},
};
use actix_web_flash_messages::FlashMessage;
use anyhow::{Context, anyhow};
use sqlx::PgPool;

use crate::{
    authentication::UserId,
    domain::{SubscriberEmail, SubscriberStatus},
    email_client::EmailClient,
    routes::error_chain_fmt,
};

#[derive(serde::Deserialize)]
pub struct PublishForm {
    title: String,
    text: String,
    html: String,
}

impl PublishForm {
    pub fn validate(&self) -> Result<(), PublishError> {
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
        HttpResponse::SeeOther()
            .insert_header((LOCATION, "/admin/newsletter"))
            .finish()
    }
}

#[tracing::instrument(name = "Publish newsletter", skip(form,pool, email_client), fields(newsletter_title=%form.title))]
pub async fn publish_newsletter(
    form: web::Form<PublishForm>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    user_id: ReqData<UserId>,
) -> Result<HttpResponse, PublishError> {
    form.validate()?;
    let confirmed_subscriptions = get_confirmed_subscriptions(&pool).await?;
    for sub in confirmed_subscriptions {
        match sub {
            Ok(s) => {
                email_client
                    .send_email(&s.email, &form.title, &form.text, &form.html)
                    .await
                    .with_context(|| format!("Failed to send email to {}", s.email))?;
            }
            Err(e) => {
                tracing::warn!("Confirmed subscriber is using invalide email {}", e)
            }
        }
    }
    FlashMessage::info("Newsletter published").send();
    Ok(HttpResponse::SeeOther()
        .insert_header((LOCATION, "/admin/dashboard"))
        .finish())
}

#[tracing::instrument(name = "Get confirmed subscriptions", skip(pool))]
pub async fn get_confirmed_subscriptions(
    pool: &PgPool,
) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
    // so here we want to query the db for confirmed subs
    let confirmed_status: String = SubscriberStatus::Confirmed.into();
    let confirmed_rows = sqlx::query!(
        r#"SELECT email FROM subscriptions WHERE status = $1"#,
        confirmed_status,
    )
    .fetch_all(pool)
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
