use sqlx::{Connection, Executor, PgConnection, PgPool};
use std::net::TcpListener;
use std::sync::LazyLock;
use uuid::Uuid;
use zero2prod::{
    configuration::{DatabaseSettings, get_configuration},
    startup,
    telementry::{get_subscriber, init_subscriber},
};

#[tokio::test]
async fn test_health_check() {
    let test_app = spawn_app().await;

    let client = reqwest::Client::new();

    let full_addr = format!("{}/health", test_app.address);
    let response = client.get(full_addr).send().await.expect("should return");

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}

#[tokio::test]
async fn subscription_returns_400_for_invalid_form_data() {
    let test_app = spawn_app().await;

    let client = reqwest::Client::new();

    let full_addr = format!("{}/subscribe", test_app.address);

    let test_cases = vec![
        ("name=jack%20cordery", "missing email"),
        ("email=jack%40gmail.com", "missing name"),
        ("", "missing both email and name"),
    ];

    for (invalid_body, test_name) in test_cases {
        let response = client
            .post(&full_addr)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(invalid_body)
            .send()
            .await
            .expect("should return");

        assert_eq!(
            400,
            response.status().as_u16(),
            "The api failed to deliver a status code of 400 whilst testing {}",
            test_name
        );
    }
}

#[tokio::test]
async fn subscribe_returns_200_for_valid_form_data() {
    let test_app = spawn_app().await;
    let full_addr = format!("{}/subscribe", test_app.address);

    let client = reqwest::Client::new();
    let response = client
        .post(&full_addr)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body("name=jack%20cordery&email=jack%40gmail.com")
        .send()
        .await
        .expect("should return");

    assert_eq!(200, response.status().as_u16());

    let saved = sqlx::query!("SELECT email, name FROM subscriptions",)
        .fetch_one(&test_app.connection_pool)
        .await
        .expect("Failed to query connection");

    assert_eq!(saved.email, "jack@gmail.com");
    assert_eq!(saved.name, "jack cordery");
}

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

async fn spawn_app() -> TestApp {
    LazyLock::force(&TRACING);

    let addr = "127.0.0.1";
    let listener = TcpListener::bind(format!("{addr}:0")).expect("should bind");
    let port = listener.local_addr().expect("should be valid").port();

    let mut configuration = get_configuration().expect("Failed to load configuration");
    configuration.database.database_name = Uuid::new_v4().to_string();
    println!("{}", configuration.database.database_name);
    let connection_pool = configure_database(&configuration.database).await;

    let server = startup::run(listener, connection_pool.clone()).expect("should spin up");
    tokio::spawn(server);
    TestApp {
        address: format!("http://{addr}:{port}"),
        connection_pool,
    }
}

async fn configure_database(db_settings: &DatabaseSettings) -> PgPool {
    let psql_connection_uri_without_db = db_settings.get_connection_uri_without_db_name();
    let psql_connection_uri_with_db = db_settings.get_connection_uri();
    let mut connection = PgConnection::connect(&psql_connection_uri_without_db)
        .await
        .expect("Failed to connect to Postgres");

    connection
        .execute(format!(r#"CREATE DATABASE "{}";"#, db_settings.database_name).as_str())
        .await
        .expect("Failed to create database");

    let connection_pool = PgPool::connect(&psql_connection_uri_with_db)
        .await
        .expect("Failed to connect to Postgres");

    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("Failed to migrate database");

    connection_pool
}
