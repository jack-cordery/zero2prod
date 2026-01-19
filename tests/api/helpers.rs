use fake::{Fake, Faker};
use secrecy::SecretString;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use std::net::TcpListener;
use std::sync::LazyLock;
use url::Url;
use uuid::Uuid;
use zero2prod::{
    configuration::{DatabaseSettings, get_configuration},
    email_client::EmailClient,
    startup,
    telementry::{get_subscriber, init_subscriber},
};

static TRACING: LazyLock<()> = LazyLock::new(|| {
    let default_name = "test".to_string();
    let default_level = "info".to_string();

    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_subscriber(default_name, default_level, std::io::stdout);
        init_subscriber(subscriber);
    } else {
        let subscriber = get_subscriber(default_name, default_level, std::io::sink);
        init_subscriber(subscriber);
    }
});

pub struct TestApp {
    pub address: String,
    pub connection_pool: PgPool,
}

pub async fn spawn_app() -> TestApp {
    LazyLock::force(&TRACING);

    let addr = "127.0.0.1";
    let listener = TcpListener::bind(format!("{addr}:0")).expect("should bind");
    let port = listener.local_addr().expect("should be valid").port();

    let mut configuration = get_configuration().expect("Failed to load configuration");
    configuration.database.database_name = Uuid::new_v4().to_string();
    println!("{}", configuration.database.database_name);
    let connection_pool = configure_database(&configuration.database).await;

    let sender_email = configuration
        .email_client
        .sender()
        .expect("Invalid sender email in configuration");
    let base_url =
        Url::parse(&configuration.email_client.base_url).expect("Invalid url in configuration");
    let fake_auth_token: String = Faker.fake();
    let timeout_duration = configuration.email_client.timeout();
    let email_client = EmailClient::new(
        base_url,
        sender_email,
        SecretString::new(fake_auth_token.into()),
        timeout_duration,
    );

    let server =
        startup::run(listener, connection_pool.clone(), email_client).expect("should spin up");
    tokio::spawn(server);
    TestApp {
        address: format!("http://{addr}:{port}"),
        connection_pool,
    }
}

async fn configure_database(db_settings: &DatabaseSettings) -> PgPool {
    let psql_connection_uri_without_db = db_settings.get_connection_uri_without_db_name();
    let psql_connection_uri_with_db = db_settings.get_connection_uri();
    let mut connection = PgConnection::connect_with(&psql_connection_uri_without_db)
        .await
        .expect("Failed to connect to Postgres");

    connection
        .execute(format!(r#"CREATE DATABASE "{}";"#, db_settings.database_name).as_str())
        .await
        .expect("Failed to create database");

    let connection_pool = PgPool::connect_with(psql_connection_uri_with_db)
        .await
        .expect("Failed to connect to Postgres");

    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("Failed to migrate database");

    connection_pool
}
