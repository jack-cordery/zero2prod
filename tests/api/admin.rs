use secrecy::SecretString;
use zero2prod::authentication::Credentials;

use crate::helpers::{assert_redirect_to, spawn_app};

#[tokio::test]
pub async fn dashboard_redirects_to_login_page_given_invalid_session() {
    let test_app = spawn_app().await;

    let response = test_app.get_admin_dashboard().await;

    assert_redirect_to(&response, "/login");
}

#[tokio::test]
pub async fn dashboard_returns_success_given_valid_session() {
    let test_app = spawn_app().await;

    let valid_credentials = Credentials {
        username: test_app.test_user.username.clone(),
        password: SecretString::from(test_app.test_user.password.clone()),
    };

    test_app.post_login(valid_credentials).await;

    let response = test_app.get_admin_dashboard().await;

    assert_eq!(200, response.status().as_u16());
}

#[tokio::test]
pub async fn dashboard_accesses_session_data_given_valid_session() {
    let test_app = spawn_app().await;

    let valid_credentials = Credentials {
        username: test_app.test_user.username.clone(),
        password: SecretString::from(test_app.test_user.password.clone()),
    };

    test_app.post_login(valid_credentials).await;

    let body = test_app.get_admin_dashboard().await.text().await.unwrap();

    assert!(body.contains(&test_app.test_user.username));
}
