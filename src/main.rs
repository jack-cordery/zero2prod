use sqlx::postgres::PgPoolOptions;
use std::{io::Result, net::TcpListener};
use url::Url;
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

    let sender_email = configuration
        .email_client
        .sender()
        .expect("Invalid sender email in configuration");
    let timeout = configuration.email_client.timeout();
    let base_url =
        Url::parse(&configuration.email_client.base_url).expect("Invalid url in configuration");
    let email_client = EmailClient::new(
        base_url,
        sender_email,
        configuration.email_client.authorization_token,
        timeout,
    );

    startup::run(listener, connection, email_client)?.await
}
