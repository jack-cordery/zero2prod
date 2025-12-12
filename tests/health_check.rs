use zero2prod::run;

#[tokio::test]
async fn test_health_check() {
    spawn_app();

    let client = reqwest::Client::new();

    let response = client
        .get("http://127.0.0.1:8000/health")
        .send()
        .await
        .expect("should return");

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}

fn spawn_app() {
    let server = run().expect("should spin up");
    tokio::spawn(server);
}
