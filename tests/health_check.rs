use sqlx::{Connection, PgConnection};
use std::net::TcpListener;
use zero2prod::{configuration, startup};

#[tokio::test]
async fn test_health_check() {
    let addr = spawn_app();

    let client = reqwest::Client::new();

    let full_addr = format!("{addr}/health");
    let response = client.get(full_addr).send().await.expect("should return");

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}

#[tokio::test]
async fn subscription_returns_400_for_invalid_form_data() {
    let addr = spawn_app();

    let client = reqwest::Client::new();

    let full_addr = format!("{addr}/subscriptions");

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
    let addr = spawn_app();
    let full_addr = format!("{addr}/subscriptions");

    let configuration = configuration::get_configuration().expect("Config should be provided");
    let psql_connection_uri = configuration.database.get_connection_uri();

    let client = reqwest::Client::new();
    let response = client
        .post(&full_addr)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body("name=jack%20cordery&email=jack%40gmail.com")
        .send()
        .await
        .expect("should return");

    assert_eq!(200, response.status().as_u16());

    let mut connection = PgConnection::connect(&psql_connection_uri)
        .await
        .expect("Failed to connect to Postgres");

    let saved = sqlx::query!("SELECT email, name FROM subscriptions",)
        .fetch_one(&mut connection)
        .await
        .expect("Failed to query connection");

    assert_eq!(saved.email, "jack@gmail.com");
    assert_eq!(saved.name, "jack cordery");
}

fn spawn_app() -> String {
    let addr = "127.0.0.1";
    let listener = TcpListener::bind(format!("{addr}:0")).expect("should bind");
    let port = listener.local_addr().expect("should be valid").port();
    let server = startup::run(listener).expect("should spin up");
    tokio::spawn(server);
    format!("http://{addr}:{port}")
}
