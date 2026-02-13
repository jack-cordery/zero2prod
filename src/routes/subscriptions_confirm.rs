use std::fmt::Formatter;

use actix_web::{HttpResponse, ResponseError, http::StatusCode, web};
use anyhow::Context;
use serde::Deserialize;
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;

use crate::{domain::SubscriberStatus, routes::error_chain_fmt};

#[derive(Deserialize)]
pub struct Parameters {
    subscription_token: String,
}

#[derive(thiserror::Error)]
pub enum ConfirmSubscriberError {
    #[error("There is no subscriber associated with this token")]
    UnknownTokenError,
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for ConfirmSubscriberError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for ConfirmSubscriberError {
    fn status_code(&self) -> StatusCode {
        match &self {
            Self::UnknownTokenError => StatusCode::UNAUTHORIZED,
            Self::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// so this will harbour the code for the /subscriptions_confirm end point
#[instrument(name = "Handling confirmation request", skip(parameters))]
pub async fn subscriptions_confirm(
    parameters: web::Query<Parameters>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ConfirmSubscriberError> {
    let subscriber_id = get_subscriber_from_token(&parameters.subscription_token, &pool)
        .await
        .context("Failed to get subscriber from token")?
        .ok_or(ConfirmSubscriberError::UnknownTokenError)?;

    update_status_to_confirmed(subscriber_id, &pool)
        .await
        .context("Failed to update status to confirmed")?;

    Ok(HttpResponse::Ok().finish())
}

#[instrument(
    name = "Get the subscriber id from the token",
    skip(subscription_token, pool)
)]
pub async fn get_subscriber_from_token(
    subscription_token: &str,
    pool: &PgPool,
) -> Result<Option<Uuid>, sqlx::Error> {
    let result = sqlx::query!(
        r#"SELECT subscriber_id  FROM subscription_tokens WHERE subscription_token = $1;"#,
        subscription_token,
    )
    .fetch_optional(pool)
    .await?;
    Ok(result.map(|r| r.subscriber_id))
}

#[instrument(name = "Updating status to confirmed", skip(subscriber_id, pool))]
pub async fn update_status_to_confirmed(
    subscriber_id: Uuid,
    pool: &PgPool,
) -> Result<(), sqlx::Error> {
    let confirmed_status: String = SubscriberStatus::Confirmed.into();
    sqlx::query!(
        r#"UPDATE subscriptions SET status=$1 WHERE id = $2"#,
        confirmed_status,
        subscriber_id
    )
    .execute(pool)
    .await?;
    Ok(())
}
