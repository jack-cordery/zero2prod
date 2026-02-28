use uuid::Uuid;
use wiremock::{
    Mock, ResponseTemplate,
    matchers::{method, path},
};

use crate::helpers::{ConfirmationLinks, TestApp, spawn_app};

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
async fn newsletter_gets_sent_to_confirmed_subscribers() {
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
async fn return_400_given_invalid_data() {
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

#[tokio::test]
async fn rejection_of_request_no_authorization_header() {
    // The set-up is just a request to /newsletter and we want the response to be
    // 401 and a header of www - basic "publish"
    let test_app = spawn_app().await;
    let client = reqwest::Client::new();

    let newsletter_body = serde_json::json!({"title": "Newsletter!",
        "content":{
            "text":"newsletter body as plain text",
            "html":"<p> newsletter body as html </p>"
        }
    });

    let response = client
        .post(format!("{}/newsletters", test_app.address))
        .json(&newsletter_body)
        .send()
        .await
        .expect("should return");

    assert_eq!(401, response.status().as_u16());
    assert_eq!(
        r#"Basic realm="publish""#,
        response.headers()["WWW-AUTHENTICATE"]
    );
}

#[tokio::test]
async fn invalid_password_is_rejected() {
    // call po
    let test_app = spawn_app().await;
    let client = reqwest::Client::new();

    let invalid_password = Uuid::new_v4().to_string();
    assert_ne!(test_app.test_user.password, invalid_password);

    let newsletter_body = serde_json::json!({"title": "Newsletter!",
        "content":{
            "text":"newsletter body as plain text",
            "html":"<p> newsletter body as html </p>"
        }
    });
    let response = client
        .post(format!("{}/newsletters", test_app.address))
        .basic_auth(&test_app.test_user.username, Some(&invalid_password))
        .json(&newsletter_body)
        .send()
        .await
        .expect("should return");
    assert_eq!(401, response.status().as_u16());
    assert_eq!(
        r#"Basic realm="publish""#,
        response.headers()["WWW-AUTHENTICATE"]
    );
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
