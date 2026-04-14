use std::time::Duration;

use serde_json::json;
use wiremock::{
    Mock, ResponseTemplate,
    matchers::{method, path},
};

use crate::helpers::{assert_redirect_to, spawn_app};

#[tokio::test]
async fn no_unconfirmed_subscribers_are_sent_newsletter() {
    let test_app = spawn_app().await;

    test_app.create_unconfirmed_subscriber().await;

    Mock::given(method("POST"))
        .and(path("/email"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&test_app.email_server)
        .await;

    // now request a send of a newsletter and the assert is checked on drop
    let title = "Newsletter!";
    let html = "<p> newsletter body as html </p>";
    let text = "newsletter body as plain text";

    let newsletter_body = json!({
        "title": title,
        "html": html,
        "text": text,
        "idempotency_key": uuid::Uuid::new_v4().to_string(),
    });

    let login_body = json!( {
        "username": test_app.test_user.username.clone(),
        "password": test_app.test_user.password.clone(),
    });

    test_app.post_login(&login_body).await;

    let response = test_app.post_newsletter(&newsletter_body).await;

    assert_redirect_to(&response, "/admin/dashboard");
}

#[tokio::test]
async fn user_must_be_logged_in_to_see_newsletter_page() {
    let test_app = spawn_app().await;

    let response = test_app.get_newsletter().await;

    assert_redirect_to(&response, "/login")
}

#[tokio::test]
async fn invalid_form_redirects_and_returns_flash_message() {
    let test_app = spawn_app().await;

    test_app.create_confirmed_subscriber().await;

    Mock::given(method("POST"))
        .and(path("/email"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&test_app.email_server)
        .await;

    // now request a send of a newsletter and the assert is checked on drop
    let test_cases = vec![
        ("empty title", "", "some_html", "some_text"),
        ("empty html", "some title", "", "some_text"),
        ("empty text", "some_title", "some_html", ""),
        ("empty text and html", "some_title", "", ""),
        ("completely empty", "", "", ""),
    ];

    let login_body = json!( {
        "username": test_app.test_user.username.clone(),
        "password": test_app.test_user.password.clone(),
    });

    test_app.post_login(&login_body).await;

    for (test_name, title, html, text) in test_cases {
        let newsletter_body = json!({
            "title": title,
            "html": html,
            "text": text,
        "idempotency_key": uuid::Uuid::new_v4().to_string(),
        });
        let response = test_app.post_newsletter(&newsletter_body).await;

        assert_eq!(303, response.status().as_u16());
        assert_eq!(
            Some("/admin/newsletter"),
            response
                .headers()
                .get("LOCATION")
                .map(|h| h.to_str().expect("invalid string")),
            "Failed redirect for test case: {}",
            test_name
        );

        let newsletter_html = test_app.get_newsletter_html().await;

        assert!(
            newsletter_html.contains("Invalid form details provided."),
            "Failed flash message for test case: {}",
            test_name
        );
    }
}

#[tokio::test]
async fn newsletter_gets_sent_to_confirmed_subscribers() {
    let test_app = spawn_app().await;

    test_app.create_confirmed_subscriber().await;

    Mock::given(method("POST"))
        .and(path("/email"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&test_app.email_server)
        .await;

    let title = "Newsletter!";
    let html = "<p> newsletter body as html </p>";
    let text = "newsletter body as plain text";

    let newsletter_body = json!({
        "title": title,
        "html": html,
        "text": text,
        "idempotency_key": uuid::Uuid::new_v4().to_string(),
    });

    let login_body = json!( {
        "username": test_app.test_user.username.clone(),
        "password": test_app.test_user.password.clone(),
    });

    test_app.post_login(&login_body).await;
    let response = test_app.post_newsletter(&newsletter_body).await;

    assert_redirect_to(&response, "/admin/dashboard");

    test_app.dispatch_all_emails().await;
}

#[tokio::test]
async fn rejection_of_publish_if_not_logged_in() {
    let test_app = spawn_app().await;

    let title = "Newsletter!".to_string();
    let html = "<p> newsletter body as html </p>".to_string();
    let text = "newsletter body as plain text".to_string();

    let newsletter_body = json!({
        "title": title,
        "html": html,
        "text": text,
        "idempotency_key": uuid::Uuid::new_v4().to_string(),
    });
    let response = test_app.post_newsletter(&newsletter_body).await;

    assert_redirect_to(&response, "/login");
}

#[tokio::test]
async fn newsletter_creation_is_idempotent() {
    let test_app = spawn_app().await;

    test_app.create_confirmed_subscriber().await;

    Mock::given(method("POST"))
        .and(path("/email"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&test_app.email_server)
        .await;

    let title = "Newsletter!";
    let html = "<p> newsletter body as html </p>";
    let text = "newsletter body as plain text";

    let newsletter_body = json!({
        "title": title,
        "html": html,
        "text": text,
        "idempotency_key": uuid::Uuid::new_v4().to_string(),
    });

    let login_body = json!( {
        "username": test_app.test_user.username.clone(),
        "password": test_app.test_user.password.clone(),
    });

    test_app.post_login(&login_body).await;

    // send once
    let response = test_app.post_newsletter(&newsletter_body).await;
    assert_redirect_to(&response, "/admin/dashboard");

    let publish_html = test_app.get_newsletter_html().await;
    assert!(
        publish_html.contains("Newsletter published"),
        "first response doesn't include success message"
    );

    // resend
    let response = test_app.post_newsletter(&newsletter_body).await;
    assert_redirect_to(&response, "/admin/dashboard");

    let publish_html = test_app.get_newsletter_html().await;
    assert!(
        publish_html.contains("Newsletter published"),
        "second response doesn't include success message"
    );

    // assert that only one email is sent - envoked on drop
    test_app.dispatch_all_emails().await;
}

#[tokio::test]
async fn newsletter_creation_is_idempotent_under_concurrent_requests() {
    let test_app = spawn_app().await;

    test_app.create_confirmed_subscriber().await;

    Mock::given(method("POST"))
        .and(path("/email"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(2)))
        .expect(1)
        .mount(&test_app.email_server)
        .await;

    let title = "Newsletter!";
    let html = "<p> newsletter body as html </p>";
    let text = "newsletter body as plain text";

    let newsletter_body = json!({
        "title": title,
        "html": html,
        "text": text,
        "idempotency_key": uuid::Uuid::new_v4().to_string(),
    });

    let login_body = json!( {
        "username": test_app.test_user.username.clone(),
        "password": test_app.test_user.password.clone(),
    });

    test_app.post_login(&login_body).await;

    let initial_request = test_app.post_newsletter(&newsletter_body);
    let retry_request = test_app.post_newsletter(&newsletter_body);

    let (initial_response, retry_response) = tokio::join!(initial_request, retry_request);

    assert_eq!(
        initial_response
            .headers()
            .get("location")
            .unwrap()
            .to_str()
            .unwrap(),
        retry_response
            .headers()
            .get("location")
            .unwrap()
            .to_str()
            .unwrap(),
    );

    assert_eq!(
        initial_response.status().as_u16(),
        retry_response.status().as_u16()
    );
    assert_eq!(
        initial_response.text().await.unwrap(),
        retry_response.text().await.unwrap()
    );

    // assert that only one email is sent - envoked on drop
    test_app.dispatch_all_emails().await;
}

#[tokio::test]
async fn transient_errors_dont_cause_duplicate_emails_on_retry() {
    let test_app = spawn_app().await;

    test_app.create_confirmed_subscriber().await;
    test_app.create_confirmed_subscriber().await;

    Mock::given(method("POST"))
        .and(path("/email"))
        .respond_with(ResponseTemplate::new(200))
        .up_to_n_times(1)
        .expect(1)
        .named("first email succesfully sent")
        .mount(&test_app.email_server)
        .await;

    Mock::given(method("POST"))
        .and(path("/email"))
        .respond_with(ResponseTemplate::new(500))
        .up_to_n_times(1)
        .expect(1)
        .named("transient error on second email")
        .mount(&test_app.email_server)
        .await;

    Mock::given(method("POST"))
        .and(path("/email"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .named("retry should only receive one request")
        .mount(&test_app.email_server)
        .await;

    let title = "Newsletter!";
    let html = "<p> newsletter body as html </p>";
    let text = "newsletter body as plain text";

    let newsletter_body = json!({
        "title": title,
        "html": html,
        "text": text,
        "idempotency_key": uuid::Uuid::new_v4().to_string(),
    });

    let login_body = json!( {
        "username": test_app.test_user.username.clone(),
        "password": test_app.test_user.password.clone(),
    });

    test_app.post_login(&login_body).await;

    let initial_request = test_app.post_newsletter(&newsletter_body);
    let retry_request = test_app.post_newsletter(&newsletter_body);

    let (initial_response, retry_response) = tokio::join!(initial_request, retry_request);

    assert_eq!(
        initial_response
            .headers()
            .get("location")
            .unwrap()
            .to_str()
            .unwrap(),
        retry_response
            .headers()
            .get("location")
            .unwrap()
            .to_str()
            .unwrap(),
    );

    assert_eq!(
        initial_response.status().as_u16(),
        retry_response.status().as_u16()
    );
    assert_eq!(
        initial_response.text().await.unwrap(),
        retry_response.text().await.unwrap()
    );

    // assert that only one email is sent - envoked on drop
    test_app.dispatch_all_emails().await;
}

#[tokio::test]
async fn publish_newsletter_will_make_no_more_than_n_attempts_to_send_email() {
    let test_app = spawn_app().await;

    test_app.create_confirmed_subscriber().await;

    Mock::given(method("POST"))
        .and(path("/email"))
        .respond_with(ResponseTemplate::new(500))
        .up_to_n_times(5)
        .expect(5)
        .named("5 failed attempts")
        .mount(&test_app.email_server)
        .await;

    Mock::given(method("POST"))
        .and(path("/email"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .named("expect no more as number of retries used up")
        .mount(&test_app.email_server)
        .await;

    let title = "Newsletter!";
    let html = "<p> newsletter body as html </p>";
    let text = "newsletter body as plain text";

    let newsletter_body = json!({
        "title": title,
        "html": html,
        "text": text,
        "idempotency_key": uuid::Uuid::new_v4().to_string(),
    });

    let login_body = json!( {
        "username": test_app.test_user.username.clone(),
        "password": test_app.test_user.password.clone(),
    });

    test_app.post_login(&login_body).await;

    test_app.post_newsletter(&newsletter_body).await;

    // assert that only one email is sent - envoked on drop
    test_app.dispatch_all_emails().await;
}

#[tokio::test]
async fn publish_newsletter_will_make_n_attempts_to_send_email() {
    let test_app = spawn_app().await;
    test_app.create_confirmed_subscriber().await;

    Mock::given(method("POST"))
        .and(path("/email"))
        .respond_with(ResponseTemplate::new(500))
        .up_to_n_times(4)
        .expect(4)
        .named("4 failed attempts")
        .mount(&test_app.email_server)
        .await;

    Mock::given(method("POST"))
        .and(path("/email"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .named("expect to see one succesfull attempt and no more after")
        .mount(&test_app.email_server)
        .await;

    let title = "Newsletter!";
    let html = "<p> newsletter body as html </p>";
    let text = "newsletter body as plain text";

    let newsletter_body = json!({
        "title": title,
        "html": html,
        "text": text,
        "idempotency_key": uuid::Uuid::new_v4().to_string(),
    });

    let login_body = json!( {
        "username": test_app.test_user.username.clone(),
        "password": test_app.test_user.password.clone(),
    });

    test_app.post_login(&login_body).await;

    test_app.post_newsletter(&newsletter_body).await;

    // assert that only one email is sent - envoked on drop
    test_app.dispatch_all_emails().await;
}
