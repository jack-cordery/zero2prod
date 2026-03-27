use serde_json::json;
use wiremock::{
    Mock, ResponseTemplate,
    matchers::{method, path},
};

use crate::helpers::{ConfirmationLinks, TestApp, assert_redirect_to, spawn_app};

#[tokio::test]
async fn no_unconfirmed_subscribers_are_sent_newsletter() {
    let test_app = spawn_app().await;

    create_unconfirmed_subscriber(&test_app).await;

    Mock::given(method("POST"))
        .and(path("/email"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&test_app.email_server)
        .await;

    // now request a send of a newsletter and the assert is checked on drop
    let title = "Newsletter!".to_string();
    let html = "<p> newsletter body as html </p>".to_string();
    let text = "newsletter body as plain text".to_string();

    let body = json!( {
        "username": test_app.test_user.username.clone(),
        "password": test_app.test_user.password.clone(),
    });

    test_app.post_login(&body).await;

    let response = test_app.post_newsletter(title, text, html).await;

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

    create_confirmed_subscriber(&test_app).await;

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

    let body = json!( {
        "username": test_app.test_user.username.clone(),
        "password": test_app.test_user.password.clone(),
    });

    test_app.post_login(&body).await;

    for (test_name, title, html, text) in test_cases {
        let response = test_app
            .post_newsletter(title.to_string(), text.to_string(), html.to_string())
            .await;

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
async fn unexpected_error_redirects_and_returns_flash_message() {
    let test_app = spawn_app().await;

    create_confirmed_subscriber(&test_app).await;

    Mock::given(method("POST"))
        .and(path("/email"))
        .respond_with(ResponseTemplate::new(500))
        .expect(1)
        .mount(&test_app.email_server)
        .await;

    let body = json!( {
        "username": test_app.test_user.username.clone(),
        "password": test_app.test_user.password.clone(),
    });

    test_app.post_login(&body).await;

    let title = "title";
    let text = "text";
    let html = "html";

    let response = test_app
        .post_newsletter(title.into(), text.into(), html.into())
        .await;

    assert_redirect_to(&response, "/admin/newsletter");

    let newsletter_html = test_app.get_newsletter_html().await;

    dbg!(&newsletter_html);

    assert!(newsletter_html.contains("An unexpected error occurred. Please try again."));
}
#[tokio::test]
async fn newsletter_gets_sent_to_confirmed_subscribers() {
    let test_app = spawn_app().await;

    create_confirmed_subscriber(&test_app).await;

    Mock::given(method("POST"))
        .and(path("/email"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&test_app.email_server)
        .await;

    let title = "Newsletter!".to_string();
    let html = "<p> newsletter body as html </p>".to_string();
    let text = "newsletter body as plain text".to_string();

    let body = json!( {
        "username": test_app.test_user.username.clone(),
        "password": test_app.test_user.password.clone(),
    });

    test_app.post_login(&body).await;
    let response = test_app.post_newsletter(title, text, html).await;

    assert_redirect_to(&response, "/admin/dashboard");
}

#[tokio::test]
async fn rejection_of_publish_if_not_logged_in() {
    let test_app = spawn_app().await;

    let title = "Newsletter!".to_string();
    let html = "<p> newsletter body as html </p>".to_string();
    let text = "newsletter body as plain text".to_string();
    let response = test_app.post_newsletter(title, text, html).await;

    assert_redirect_to(&response, "/login");
}

async fn create_unconfirmed_subscriber(test_app: &TestApp) -> ConfirmationLinks {
    let _mock_guard = Mock::given(method("POST"))
        .and(path("/email"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .named("Create unconfirmed subscriber")
        .mount_as_scoped(&test_app.email_server)
        .await;

    test_app
        .post_subsription("name=jack%20cordery&email=jack%40gmail.com".into())
        .await
        .error_for_status()
        .unwrap();

    let recieved_request = &test_app
        .email_server
        .received_requests()
        .await
        .unwrap()
        .pop()
        .unwrap();
    test_app.get_confirmation_links(recieved_request)
}

async fn create_confirmed_subscriber(test_app: &TestApp) {
    let confirmation_links = create_unconfirmed_subscriber(test_app).await;
    reqwest::get(confirmation_links.html)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
}
