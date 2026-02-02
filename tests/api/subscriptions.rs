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
async fn subscribe_fails_if_there_is_a_fatal_database_error() {
    // we will enforce the database returning a falter error
    // by first deleting a critical column from the db and the calling Post subscriber
    // then asserting that it returns status 500

    let test_app = spawn_app().await;

    sqlx::query!("ALTER TABLE subscription_tokens DROP COLUMN subscription_token")
        .execute(&test_app.connection_pool)
        .await
        .expect("Failed to execute query.");

    let response = test_app
        .post_subsription("name=jack%20cordery&email=jack%40gmail.com".into())
        .await;

    assert_eq!(500, response.status().as_u16());
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

    test_app
        .post_subsription("name=jack%20cordery&email=jack%40gmail.com".into())
        .await;

    let recieved_request = &test_app.email_server.received_requests().await.unwrap()[0];

    let confirmation_links = test_app.get_confirmation_links(recieved_request);

    assert_eq!(confirmation_links.html, confirmation_links.text);
}

#[tokio::test]
async fn unconfirmed_resubscription_returns_200() {
    let test_app = spawn_app().await;

    Mock::given(method("POST"))
        .and(path("/email"))
        .respond_with(ResponseTemplate::new(200))
        .expect(2) // expect two emails
        .mount(&test_app.email_server)
        .await;

    test_app
        .post_subsription("name=jack%20cordery&email=jack%40gmail.com".into())
        .await;

    let response = test_app
        .post_subsription("name=jack%20cordery&email=jack%40gmail.com".into())
        .await;
    assert_eq!(200, response.status().as_u16());
}
#[tokio::test]
async fn confirmed_resubscription_returns_409() {
    let test_app = spawn_app().await;

    Mock::given(method("POST"))
        .and(path("/email"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1) // dont expect a second email request
        .mount(&test_app.email_server)
        .await;

    test_app
        .post_subsription("name=jack%20cordery&email=jack%40gmail.com".into())
        .await;

    let recieved_request = &test_app.email_server.received_requests().await.unwrap()[0];

    let confirmation_links = test_app.get_confirmation_links(recieved_request);

    reqwest::get(confirmation_links.html)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let response = test_app
        .post_subsription("name=jack%20cordery&email=jack%40gmail.com".into())
        .await;
    assert_eq!(409, response.status().as_u16());
}

#[tokio::test]
async fn unconfirmed_resubscription_emails_same_link_twice() {
    let test_app = spawn_app().await;

    Mock::given(method("POST"))
        .and(path("/email"))
        .respond_with(ResponseTemplate::new(200))
        .expect(2) // expect two emails
        .mount(&test_app.email_server)
        .await;

    test_app
        .post_subsription("name=jack%20cordery&email=jack%40gmail.com".into())
        .await;

    test_app
        .post_subsription("name=jack%20cordery&email=jack%40gmail.com".into())
        .await;

    let first_email = &test_app.email_server.received_requests().await.unwrap()[0];
    let second_email = &test_app.email_server.received_requests().await.unwrap()[1];

    let first_confirmation_links = test_app.get_confirmation_links(first_email);
    let second_confirmation_links = test_app.get_confirmation_links(second_email);
    assert_eq!(first_confirmation_links, second_confirmation_links);
}
