use secrecy::SecretString;
use zero2prod::authentication::Credentials;

use crate::helpers::{assert_redirect_to, spawn_app};

#[tokio::test]
pub async fn a_flash_error_message_is_returned_on_failure() {
    // Arrange
    let app = spawn_app().await;

    // Act
    let invalid_credentials = Credentials {
        username: "random-username".into(),
        password: SecretString::new("random-password".into()),
    };
    let response = app.post_login(invalid_credentials).await;

    // Assert
    assert_redirect_to(&response, "/login");

    // Act - Part 2 - Follow the redirect
    let html_page = app.get_login_html().await;
    assert!(html_page.contains("Authentication failed"));

    // Act - Part 3 - Reload the login page
    let html_page = app.get_login_html().await;
    assert!(!html_page.contains("Authentication failed"));
}
