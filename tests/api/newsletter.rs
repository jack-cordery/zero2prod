use wiremock::{
    Mock, ResponseTemplate,
    matchers::{method, path},
};

use crate::helpers::{ConfirmationLinks, TestApp, spawn_app};

#[tokio::test]
async fn test_no_unconfirmed_subscribers_are_sent_newsletter() {
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

    let response = test_app.post_newsletter(newsletter_body).await;

    assert_eq!(200, response.status().as_u16());
}

#[tokio::test]
async fn test_newsletter_gets_sent_to_confirmed_subscribers() {
    // here we will want to create
    let test_app = spawn_app().await;

    create_confirmed_subscriber(&test_app).await;

    Mock::given(method("POST"))
        .and(path("/email"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&test_app.email_server)
        .await;

    // now request a send of a newsletter and the assert is checked on drop
    let newsletter_body = serde_json::json!({"title": "Newsletter!",
        "content":{
            "text":"newsletter body as plain text",
            "html":"<p> newsletter body as html </p>"
        }
    });

    let response = test_app.post_newsletter(newsletter_body).await;

    assert_eq!(200, response.status().as_u16());
}

#[tokio::test]
async fn test_return_400_given_invalid_data() {
    let test_cases = vec![
        (
            serde_json::json!({
                "content":{
                    "text": "some test",
                    "html": "some html",
                }
            }),
            "missing title",
        ),
        (
            serde_json::json!({
                "content":{
                    "text": "some test",
                },
                    "title": "some title",
            }),
            "missing html content",
        ),
        (
            serde_json::json!({
                "content":{
                    "html": "some test",
                },
                    "title": "some title",
            }),
            "missing text content",
        ),
        (
            serde_json::json!({"title": "some title"}),
            "missing content",
        ),
    ];

    let test_app = spawn_app().await;

    for (test_body, test_name) in test_cases {
        let response = test_app.post_newsletter(test_body).await;

        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not fail with 400 when the payload was {}",
            test_name
        )
    }
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
