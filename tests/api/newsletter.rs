use wiremock::{
    Mock, ResponseTemplate,
    matchers::{method, path},
};

use crate::helpers::{TestApp, spawn_app};

#[tokio::test]
async fn test_no_unconfirmed_subscribers_are_sent_newsletter() {
    // we want to create a test that
    // will ensure that when we call the
    // end point that will distribute a newsletter
    // that we ensure that no unconfirmed subscribers get sent one
    // as we will be sending the newsletter using postmark
    // we can just set-up by creatiing a unsubscribed subscriber
    // and then just ensuring that we recieve 0 requests to send an email over
    // postmark api which will be mocked

    // setup
    // - spawn app
    // - mock postmark
    // - create subscriber
    let test_app = spawn_app().await;

    create_unconfirmed_subscriber(&test_app).await;

    Mock::given(method("POST"))
        .and(path("/email"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&test_app.email_server)
        .await;

    // now request a send of a newsletter and the assert is checked on drop
    let newsletter_body = serde_json::json!({"title": "Newsletter!",
        "content":{
            "text":"newsletter body as plain text",
            "html":"<p> newsletter body as html </p>"
        }
    });

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/newsletters", &test_app.address))
        .json(&newsletter_body)
        .send()
        .await
        .expect("should return");

    assert_eq!(200, response.status().as_u16());
}

async fn create_unconfirmed_subscriber(test_app: &TestApp) {
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
}
