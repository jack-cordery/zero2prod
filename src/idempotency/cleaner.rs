use std::{sync::Arc, time::Duration};

use sqlx::PgPool;
use tokio::time::sleep;
use tracing::instrument;

use crate::{configuration::Settings, startup::get_connection_pool};

pub async fn run_until_stopped(settings: Arc<Settings>) -> Result<(), anyhow::Error> {
    // so to run this all we need is a connection pool for access to the databse
    //
    let pool = get_connection_pool(&settings.database);
    worker_loop(&settings, &pool).await
}

// so we want a worker that will periodically query the idempotency key table
// and delete all of the keys with a creation date greater than n days ago!
//
// run_until_stopped() - which willl get the config and do some stuff and run worker loop
// worker_loop() - this will be a loop {clean_up() => } we want it to run on start-up and then
// run every n days, which is specified by the config.
//
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
