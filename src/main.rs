use reqwest::Client;
use sqlx::postgres::PgPoolOptions;
use std::{io::Result, net::TcpListener};
use zero2prod::{
    configuration::get_configuration,
    email_client::EmailClient,
    startup,
    telementry::{get_subscriber, init_subscriber},
};

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);

    init_subscriber(subscriber);

    let configuration = get_configuration().expect("Failed to read configuration");
    let application_address = format!(
        "{}:{}",
        configuration.application.host, configuration.application.port
    );
    let listener = TcpListener::bind(application_address)?;

    let psql_connection_uri = configuration.database.get_connection_uri();
    let connection = PgPoolOptions::new().connect_lazy_with(psql_connection_uri);

    let reqwest_client = Client::new();
    let sender_email = configuration
        .email_client
        .sender()
        .expect("Invalid sender email in configuration");
    let email_client = EmailClient::new(
        reqwest_client,
        configuration.email_client.base_url,
        sender_email,
    );

    startup::run(listener, connection, email_client)?.await
}
