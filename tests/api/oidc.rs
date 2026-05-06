use serde_json::json;
use wiremock::{
    Mock, ResponseTemplate,
    matchers::{method, path},
};

use crate::helpers::{
    assert_redirect_to, get_callback_query_from_login_response, make_test_jwt, spawn_app,
};

#[tokio::test]
async fn login_with_google_redirects_to_correct_end_point() {
    let test_app = spawn_app().await;

    let response = test_app.get_google_login().await;

    let oidc_uri = test_app.oidc_server.uri();
    let expected_redirect_location = format!("{oidc_uri}/auth");

    let mut redirect_location = response
        .headers()
        .get("LOCATION")
        .map(|h| url::Url::parse(h.to_str().expect("invalid string")).expect("Invalid url"))
        .expect("no location header");

    redirect_location.set_query(None); // Want to equate the base url

    assert_eq!(redirect_location.as_str(), &expected_redirect_location);
}

#[tokio::test]
async fn user_is_logged_in_if_they_successfully_auth_with_google() {
    let test_app = spawn_app().await;

    let initiate_login_response = test_app.get_google_login().await;
    let (callback_query, nonce) = get_callback_query_from_login_response(initiate_login_response);

    dbg!(&callback_query);

    let claims = json!( {
        "iss": test_app.oidc_server.uri(),
        "sub": "12345",
        "aud": "some_client_id",
        "iat": 1700000000,
        "exp": 2000000000,
        "nonce": nonce,
        "email": test_app.test_user.username
    });

    let id_token = make_test_jwt(json!(claims));
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "some_access_token",
            "token_type": "Bearer",
            "id_token": id_token,
        })))
        .expect(1)
        .mount(&test_app.oidc_server)
        .await;

    let response = test_app.get_google_callback(callback_query).await;
    assert_redirect_to(&response, "/admin/dashboard");
}

#[tokio::test]
async fn user_is_not_logged_in_if_they_callback_with_an_email_that_isnt_an_admin() {
    let test_app = spawn_app().await;

    let initiate_login_response = test_app.get_google_login().await;
    let (callback_query, nonce) = get_callback_query_from_login_response(initiate_login_response);

    let claims = json!( {
        "iss": test_app.oidc_server.uri(),
        "sub": "12345",
        "aud": "some_client_id",
        "iat": 1700000000,
        "exp": 2000000000,
        "nonce": nonce,
        "email": "some_other_email"
    });

    let id_token = make_test_jwt(json!(claims));
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "some_access_token",
            "token_type": "Bearer",
            "id_token": id_token,
        })))
        .expect(1)
        .mount(&test_app.oidc_server)
        .await;

    let response = test_app.get_google_callback(callback_query).await;
    assert_redirect_to(&response, "/login");
}

#[tokio::test]
async fn user_is_not_logged_in_if_they_callback_with_a_different_nonce() {
    let test_app = spawn_app().await;

    let initiate_login_response = test_app.get_google_login().await;
    let (callback_query, _) = get_callback_query_from_login_response(initiate_login_response);

    let claims = json!( {
        "iss": test_app.oidc_server.uri(),
        "sub": "12345",
        "aud": "some_client_id",
        "iat": 1700000000,
        "exp": 2000000000,
        "nonce": "some_other_nonce",
        "email": test_app.test_user.username,
    });

    let id_token = make_test_jwt(json!(claims));
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "some_access_token",
            "token_type": "Bearer",
            "id_token": id_token,
        })))
        .expect(1)
        .mount(&test_app.oidc_server)
        .await;

    let response = test_app.get_google_callback(callback_query).await;
    assert_redirect_to(&response, "/login");
}
