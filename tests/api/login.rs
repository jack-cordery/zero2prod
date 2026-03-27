use reqwest::header::SET_COOKIE;
use serde_json::json;

use crate::helpers::{assert_redirect_to, spawn_app};

#[tokio::test]
pub async fn a_flash_error_message_is_returned_on_failure() {
    // Arrange
    let test_app = spawn_app().await;

    // Act

    let body = json!({
        "username": "random-username",
        "password": "random-password",
    });
    let response = test_app.post_login(&body).await;

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

    let body = json!( {
        "username": test_app.test_user.username.clone(),
        "password": test_app.test_user.password.clone(),
    });

    let response = test_app.post_login(&body).await;

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
