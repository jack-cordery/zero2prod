use std::{sync::Arc, time::Duration};

use sqlx::PgPool;
use tokio::time::sleep;
use tracing::instrument;

use crate::{configuration::Settings, startup::get_connection_pool};

pub async fn run_until_stopped(settings: Arc<Settings>) -> Result<(), anyhow::Error> {
    let pool = get_connection_pool(&settings.database);
    worker_loop(&settings, &pool).await
}

#[instrument(name = "deleting expired keys", skip(expiry_days, pool))]
pub async fn delete_expired_keys(expiry_days: u8, pool: &PgPool) -> Result<(), anyhow::Error> {
    sqlx::query!(
        r#"DELETE FROM idempotency
WHERE created_at < now() - $1 * interval '1 days';"#,
        expiry_days as f64
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn worker_loop(settings: &Settings, pool: &PgPool) -> Result<(), anyhow::Error> {
    loop {
        delete_expired_keys(settings.application.idempotent_expiry, pool).await?;
        sleep(Duration::from_hours(24)).await;
    }
}
