use wiremock::{
    Mock, ResponseTemplate,
    matchers::{method, path},
};
use zero2prod::domain::SubscriberStatus;

use crate::helpers::spawn_app;

#[tokio::test]
async fn subscription_confirm_rejects_requests_without_query_param() {
    let test_app = spawn_app().await;
    let link_without_query = format!("{}/subscriptions/confirm", test_app.address);
    let response = reqwest::get(link_without_query).await.unwrap();
    assert_eq!(400, response.status().as_u16())
}

#[tokio::test]
async fn the_link_returned_by_subscribe_returns_200_if_called() {
    let test_app = spawn_app().await;

    Mock::given(method("POST"))
        .and(path("/email"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&test_app.email_server)
        .await;

    test_app
        .post_subsription("name=jack%20cordery&email=jack%40gmail.com".into())
        .await;

    let recieved_email_request = &test_app.email_server.received_requests().await.unwrap()[0];

    let confirmation_links = test_app.get_confirmation_links(recieved_email_request);

    let html_response = reqwest::get(confirmation_links.html).await.unwrap();
    let text_response = reqwest::get(confirmation_links.text).await.unwrap();

    assert_eq!(html_response.status().as_u16(), 200);
    assert_eq!(text_response.status().as_u16(), 200);
}

#[tokio::test]
async fn clicking_on_confirmation_link_updates_database() {
    let test_app = spawn_app().await;

    Mock::given(method("POST"))
        .and(path("/email"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&test_app.email_server)
        .await;

    test_app
        .post_subsription("name=jack%20cordery&email=jack%40gmail.com".into())
        .await;

    let recieved_email_request = &test_app.email_server.received_requests().await.unwrap()[0];

    let confirmation_links = test_app.get_confirmation_links(recieved_email_request);

    reqwest::get(confirmation_links.html)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let saved = sqlx::query!("SELECT email, name, status FROM subscriptions")
        .fetch_one(&test_app.connection_pool)
        .await
        .unwrap();

    let confirmed_status: String = SubscriberStatus::Confirmed.into();
    assert_eq!(saved.email, "jack@gmail.com");
    assert_eq!(saved.name, "jack cordery");
    assert_eq!(saved.status, confirmed_status);
}

#[tokio::test]
async fn confirm_subscriber_fails_if_there_is_a_fatal_database_error() {
    // we will enforce the database returning a falter error
    // by first deleting a critical column from the db and the calling Post subscriber
    // then asserting that it returns status 500

    let test_app = spawn_app().await;

    Mock::given(method("POST"))
        .and(path("/email"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&test_app.email_server)
        .await;

    test_app
        .post_subsription("name=jack%20cordery&email=jack%40gmail.com".into())
        .await;

    let recieved_email_request = &test_app.email_server.received_requests().await.unwrap()[0];

    let confirmation_links = test_app.get_confirmation_links(recieved_email_request);

    sqlx::query!("ALTER TABLE subscription_tokens DROP COLUMN subscription_token")
        .execute(&test_app.connection_pool)
        .await
        .expect("Failed to execute query.");

    let response = reqwest::get(confirmation_links.html).await.unwrap();

    assert_eq!(500, response.status().as_u16());
}
