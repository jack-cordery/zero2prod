use std::net;

use actix_web::{
    body::MessageBody,
    dev::{ServiceRequest, ServiceResponse},
    error::ErrorTooManyRequests,
    middleware::Next,
    web,
};
use anyhow::Context;
use redis::aio::MultiplexedConnection;
use tracing::instrument;

use crate::{configuration::RateLimitSettings, routes::error_chain_fmt, utils::e500};

#[derive(thiserror::Error)]
enum RateLimitError {
    #[error("Rate limit exceeded")]
    ExceededError,
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}
impl std::fmt::Debug for RateLimitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

async fn check_rate_limit(
    socket_address: net::SocketAddr,
    connection: &mut MultiplexedConnection,
    rate_limit_settings: &RateLimitSettings,
) -> Result<(), RateLimitError> {
    let rate_limit = rate_limit_settings.rate_limit;
    let namespace = &rate_limit_settings.namespace;
    let ip_address = socket_address.ip().to_string();
    let now = chrono::Utc::now().timestamp();
    let key = format!("{namespace}::rate_limit::{ip_address}::{now}");
    let result: Vec<i32> = redis::pipe()
        .atomic()
        .incr(&key, 1)
        .expire(&key, 10)
        .query_async(connection)
        .await
        .context("transaction not able to complete")?;

    let time_slot_count = result[0];

    if time_slot_count > rate_limit as i32 {
        Err(RateLimitError::ExceededError)
    } else {
        Ok(())
    }
}

#[instrument(
    name = "checking rate limits",
    skip(req, next, rate_limit, redis_client)
)]
pub async fn rate_limit_protection(
    redis_client: web::Data<redis::Client>,
    rate_limit: web::Data<RateLimitSettings>,
    req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<impl MessageBody>, actix_web::Error> {
    let mut connection = redis_client
        .get_multiplexed_async_connection()
        .await
        .map_err(e500)?;
    let socket_address = req
        .peer_addr()
        .context("unable to identify source")
        .map_err(e500)?;
    let rate_limit = rate_limit.get_ref();
    if check_rate_limit(socket_address, &mut connection, rate_limit)
        .await
        .is_ok()
    {
        return next.call(req).await;
    } else {
        Err(ErrorTooManyRequests("Rate limit reached"))
    }
}
