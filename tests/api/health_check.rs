use crate::helpers::spawn_app;

#[tokio::test]
async fn test_health_check() {
    let test_app = spawn_app().await;

    let client = reqwest::Client::new();

    let full_addr = format!("{}/health", test_app.address);
    let response = client.get(full_addr).send().await.expect("should return");

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}
