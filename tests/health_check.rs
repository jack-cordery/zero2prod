use std::net::TcpListener;

use zero2prod::run;

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

    let client = reqwest::Client::new();

    let full_addr = format!("{addr}/subscriptions");

    let response = client
        .post(&full_addr)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body("name=jack%20cordery&email=jack%40gmail.com")
        .send()
        .await
        .expect("should return");

    assert_eq!(200, response.status().as_u16());
}

fn spawn_app() -> String {
    let addr = "127.0.0.1";
    let listener = TcpListener::bind(format!("{addr}:0")).expect("should bind");
    let port = listener.local_addr().expect("should be valid").port();
    let server = run(listener).expect("should spin up");
    tokio::spawn(server);
    format!("http://{addr}:{port}")
}
