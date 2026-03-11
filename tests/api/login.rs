use reqwest::header::SET_COOKIE;
use secrecy::SecretString;
use zero2prod::authentication::Credentials;

use crate::helpers::{assert_redirect_to, spawn_app};

#[tokio::test]
pub async fn a_flash_error_message_is_returned_on_failure() {
    // Arrange
    let test_app = spawn_app().await;

    // Act
    let invalid_credentials = Credentials {
        username: "random-username".into(),
        password: SecretString::new("random-password".into()),
    };
    let response = test_app.post_login(invalid_credentials).await;

    // Assert
    assert_redirect_to(&response, "/login");

    // Act - Part 2 - Follow the redirect
    let html_page = test_app.get_login_html().await;
    assert!(html_page.contains("Authentication failed"));

    // Act - Part 3 - Reload the login page
    let html_page = test_app.get_login_html().await;
    assert!(!html_page.contains("Authentication failed"));
}

#[tokio::test]
pub async fn login_redirects_to_admin_dashboard_on_success() {
    let test_app = spawn_app().await;

    let test_credentials = Credentials {
        username: test_app.test_user.username.clone(),
        password: SecretString::from(test_app.test_user.password.clone()),
    };

    let response = test_app.post_login(test_credentials).await;

    let session_cookie_cookies: Vec<&str> = response
        .headers()
        .get_all(SET_COOKIE)
        .iter()
        .filter(|c| c.to_str().unwrap().contains("id="))
        .map(|c| c.to_str().unwrap())
        .collect();

    assert!(session_cookie_cookies.len() == 1);

    assert_redirect_to(&response, "/admin/dashboard");
}
