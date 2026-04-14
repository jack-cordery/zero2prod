use crate::helpers::spawn_app;
use serde_json::json;
use wiremock::{
    Mock, ResponseTemplate,
    matchers::{method, path},
};

#[tokio::test]
pub async fn issues_shows_completed_publications() {
    let test_app = spawn_app().await;
    test_app.create_confirmed_subscriber().await;

    Mock::given(method("POST"))
        .and(path("/email"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&test_app.email_server)
        .await;

    let login_body = json!( {
        "username": test_app.test_user.username.clone(),
        "password": test_app.test_user.password.clone(),
    });
    let newsletter_body = json!({
        "title": "some great title",
        "html": "content in <i>great</i> html",
        "text": "content in not-so great text",
            "idempotency_key": uuid::Uuid::new_v4().to_string(),
    });

    test_app.post_login(&login_body).await;
    test_app.post_newsletter(&newsletter_body).await;
    test_app.dispatch_all_emails().await;

    let issues_html = test_app.get_issues_html().await;

    assert!(issues_html.contains("Status: Completed"));
}

#[tokio::test]
pub async fn issues_shows_in_progress_publications() {
    let test_app = spawn_app().await;
    test_app.create_confirmed_subscriber().await;
    test_app.create_confirmed_subscriber().await;

    let login_body = json!( {
        "username": test_app.test_user.username.clone(),
        "password": test_app.test_user.password.clone(),
    });
    let newsletter_body = json!({
        "title": "some great title",
        "html": "content in <i>great</i> html",
        "text": "content in not-so great text",
            "idempotency_key": uuid::Uuid::new_v4().to_string(),
    });

    test_app.post_login(&login_body).await;
    test_app.post_newsletter(&newsletter_body).await;

    let issues_html = test_app.get_issues_html().await;
    assert!(issues_html.contains("Status: In progress, Tasks left: 2"));
}
