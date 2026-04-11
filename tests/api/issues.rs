use crate::helpers::spawn_app;
use serde_json::json;

#[tokio::test]
pub async fn issues_shows_completed_publications() {
    let test_app = spawn_app().await;
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
    test_app.dispatch_all_emails().await;

    let issues_html = test_app.get_issues_html().await;

    assert!(issues_html.contains("Status: Completed"));
}

#[tokio::test]
pub async fn issues_shows_in_progress_publications() {
    let test_app = spawn_app().await;
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
    assert!(issues_html.contains("Status: In Progress. Tasks left: 2"));
}
