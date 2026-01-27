use actix_web::{HttpResponse, web};
use serde::Deserialize;
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;

use crate::domain::SubscriberStatus;

#[derive(Deserialize)]
pub struct Parameters {
    subscription_token: String,
}

/// so this will harbour the code for the /subscriptions_confirm end point
#[instrument(name = "Handling confirmation request", skip(parameters))]
pub async fn subscriptions_confirm(
    parameters: web::Query<Parameters>,
    pool: web::Data<PgPool>,
) -> HttpResponse {
    let subscriber_id = match get_subscriber_from_token(&parameters.subscription_token, &pool).await
    {
        Ok(Some(id)) => id,
        Ok(None) => return HttpResponse::Unauthorized().finish(),
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    if update_status_to_confirmed(subscriber_id, &pool)
        .await
        .is_err()
    {
        return HttpResponse::InternalServerError().finish();
    }
    HttpResponse::Ok().finish()
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
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query {e:?}");
        e
    })?;
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
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query {e:?}");
        e
    })?;
    Ok(())
}
