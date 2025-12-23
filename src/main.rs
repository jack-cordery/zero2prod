use sqlx::PgPool;
use std::{io::Result, net::TcpListener};
use tracing::subscriber::set_global_default;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_subscriber::{EnvFilter, Registry, prelude::*};
use zero2prod::{configuration::get_configuration, startup};

#[tokio::main]
async fn main() -> Result<()> {
    LogTracer::init().expect("Failed to load log_tracer");
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let formatting_layer = BunyanFormattingLayer::new("zero2prod".into(), std::io::stdout);
    let subscriber = Registry::default()
        .with(env_filter)
        .with(JsonStorageLayer)
        .with(formatting_layer);

    set_global_default(subscriber).expect("Failed to load subscriber");

    let configuration = get_configuration().expect("Failed to read configuration");
    let application_address = format!("127.0.0.1:{}", configuration.application_port);
    let listener = TcpListener::bind(application_address)?;

    let psql_connection_uri = configuration.database.get_connection_uri();
    let connection = PgPool::connect(&psql_connection_uri)
        .await
        .expect("Failed to connect to Postgres");
    startup::run(listener, connection)?.await
}
