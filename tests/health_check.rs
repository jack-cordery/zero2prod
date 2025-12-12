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

fn spawn_app() -> String {
    let addr = "127.0.0.1";
    let listener = TcpListener::bind(format!("{addr}:0")).expect("should bind");
    let port = listener.local_addr().expect("should be valid").port();
    let server = run(listener).expect("should spin up");
    tokio::spawn(server);
    format!("http://{addr}:{port}")
}
