use serde_json::json;
use uuid::Uuid;
use wiremock::{
    Mock, ResponseTemplate,
    matchers::{method, path},
};

use crate::helpers::spawn_app;

#[tokio::test]
pub async fn cleaner_clears_expired_keys_when_called() {
    let test_app = spawn_app().await;

    test_app.create_confirmed_subscriber().await;

    Mock::given(method("POST"))
        .and(path("/email"))
        .respond_with(ResponseTemplate::new(200))
        .expect(2)
        .mount(&test_app.email_server)
        .await;

    let title = "Newsletter!";
    let html = "<p> newsletter body as html </p>";
    let text = "newsletter body as plain text";
    let idempotency_key = Uuid::new_v4().to_string();

    let newsletter_body = json!({
        "title": title,
        "html": html,
        "text": text,
        "idempotency_key": &idempotency_key,
    });

    let login_body = json!( {
        "username": test_app.test_user.username.clone(),
        "password": test_app.test_user.password.clone(),
    });

    test_app.post_login(&login_body).await;

    test_app.post_newsletter(&newsletter_body).await;
    test_app.dispatch_all_emails().await;

    test_app.clean_idempotency_keys().await;
    test_app.post_newsletter(&newsletter_body).await;
    test_app.dispatch_all_emails().await;

    // assert on drop that we got 2 emails sent out as the
    // old key was deleted and therefore seen as a new newsletter
}
