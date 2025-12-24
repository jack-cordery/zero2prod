use sqlx::PgPool;
use std::{io::Result, net::TcpListener};
use zero2prod::{
    configuration::get_configuration,
    startup,
    telementry::{get_subscriber, init_subscriber},
};

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);

    init_subscriber(subscriber);

    let configuration = get_configuration().expect("Failed to read configuration");
    let application_address = format!("127.0.0.1:{}", configuration.application_port);
    let listener = TcpListener::bind(application_address)?;

    let psql_connection_uri = configuration.database.get_connection_uri();
    let connection = PgPool::connect(&psql_connection_uri)
        .await
        .expect("Failed to connect to Postgres");
    startup::run(listener, connection)?.await
}
