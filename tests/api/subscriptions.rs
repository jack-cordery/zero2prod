use linkify::LinkFinder;
use wiremock::{
    Mock, ResponseTemplate,
    matchers::{method, path},
};

use crate::helpers::spawn_app;

#[tokio::test]
async fn subscription_returns_400_for_invalid_form_data() {
    let test_app = spawn_app().await;

    let test_cases = vec![
        ("name=jack%20cordery", "missing email"),
        ("email=jack%40gmail.com", "missing name"),
        ("", "missing both email and name"),
    ];

    for (invalid_body, test_name) in test_cases {
        let response = test_app.post_subsription(invalid_body.into()).await;

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
    let test_app = spawn_app().await;

    Mock::given(method("POST"))
        .and(path("/email"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&test_app.email_server)
        .await;

    let response = test_app
        .post_subsription("name=jack%20cordery&email=jack%40gmail.com".into())
        .await;
    assert_eq!(200, response.status().as_u16());
}

#[tokio::test]
async fn subscribe_persists_the_new_subscriber() {
    let test_app = spawn_app().await;

    Mock::given(method("POST"))
        .and(path("/email"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&test_app.email_server)
        .await;

    let _ = test_app
        .post_subsription("name=jack%20cordery&email=jack%40gmail.com".into())
        .await;

    let saved = sqlx::query!("SELECT email, name, status FROM subscriptions",)
        .fetch_one(&test_app.connection_pool)
        .await
        .expect("Failed to query connection");

    assert_eq!(saved.email, "jack@gmail.com");
    assert_eq!(saved.name, "jack cordery");
    assert_eq!(saved.status, "pending_confirmation".to_string());
}

#[tokio::test]
async fn subscribe_returns_400_for_invalid_or_missing_data() {
    let test_app = spawn_app().await;

    let test_tupes = [
        ("name=&email=some@email.com", "missing name"),
        ("name=some%20name&email=", "missing email"),
        ("name=some%20name&email=notanemail", "invalid email"),
    ];

    for (test_input, test_name) in test_tupes {
        let response = test_app.post_subsription(test_input.into()).await;

        assert_eq!(
            400,
            response.status().as_u16(),
            "The api failed to return 400 when testing {test_name}"
        );

        let saved = sqlx::query!("SELECT email, name FROM subscriptions",)
            .fetch_one(&test_app.connection_pool)
            .await;

        assert!(
            matches!(saved, Err(sqlx::Error::RowNotFound)),
            "The api failed by returning rows that should not have been writtten to DB in test: {test_name}"
        );
    }
}

#[tokio::test]
async fn subscribe_sends_confirmation_email_given_correct_inputs() {
    let test_app = spawn_app().await;

    Mock::given(method("POST"))
        .and(path("/email"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&test_app.email_server)
        .await;

    let response = test_app
        .post_subsription("name=jack%20cordery&email=jack%40gmail.com".into())
        .await;

    assert_eq!(200, response.status().as_u16());

    // assert occurs on drop of Mock
}

#[tokio::test]
async fn subscribe_sends_confirmation_email_with_a_link() {
    // so this needs to setup test app attach a mock response
    // then use recieved_requests to check the body contains the session token
    // before we do that we need to deserialize it
    let test_app = spawn_app().await;

    Mock::given(method("POST"))
        .and(path("/email"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&test_app.email_server)
        .await;

    let _ = test_app
        .post_subsription("name=jack%20cordery&email=jack%40gmail.com".into())
        .await;

    let recieved_request = &test_app.email_server.received_requests().await.unwrap()[0];
    let body: serde_json::Value = serde_json::from_slice(&recieved_request.body).unwrap();

    let get_link = |s: &str| {
        let links: Vec<_> = LinkFinder::new()
            .links(s)
            .filter(|l| *l.kind() == linkify::LinkKind::Url)
            .collect();
        assert_eq!(links.len(), 1);
        links[0].as_str().to_owned()
    };

    let html_link = get_link(body["HtmlBody"].as_str().unwrap());
    let text_link = get_link(body["TextBody"].as_str().unwrap());

    assert_eq!(html_link, text_link);
}
