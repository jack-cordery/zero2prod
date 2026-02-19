use actix_web::{HttpResponse, ResponseError, http::StatusCode, web};
use anyhow::Context;
use sqlx::PgPool;

use crate::{
    domain::{SubscriberEmail, SubscriberStatus},
    email_client::EmailClient,
    routes::error_chain_fmt,
};

#[derive(serde::Deserialize)]
pub struct BodyData {
    title: String,
    content: Content,
}
#[derive(serde::Deserialize)]
pub struct Content {
    text: String,
    html: String,
}

pub struct ConfirmedSubscriber {
    email: SubscriberEmail,
}

#[derive(thiserror::Error)]
pub enum PublishError {
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for PublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for PublishError {
    fn status_code(&self) -> StatusCode {
        match &self {
            Self::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(name = "Publish newsletter", skip(body, email_client), fields(newsletter_title=%body.title))]
pub async fn publish_newsletter(
    body: web::Json<BodyData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
) -> Result<HttpResponse, PublishError> {
    let confirmed_subscriptions = get_confirmed_subscriptions(&pool).await?;
    for sub in confirmed_subscriptions {
        match sub {
            Ok(s) => {
                email_client
                    .send_email(
                        &s.email,
                        &body.title,
                        &body.content.text,
                        &body.content.html,
                    )
                    .await
                    .with_context(|| format!("Failed to send email to {}", s.email))?;
            }
            Err(e) => {
                tracing::warn!("Confirmed subscriber is using invalide email {}", e)
            }
        }
    }
    Ok(HttpResponse::Ok().finish())
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
