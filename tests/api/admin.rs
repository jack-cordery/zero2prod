use secrecy::SecretString;
use zero2prod::authentication::{Credentials, validate_credentials};

use crate::helpers::{assert_redirect_to, spawn_app};

#[tokio::test]
pub async fn dashboard_redirects_to_login_page_given_invalid_session() {
    let test_app = spawn_app().await;

    let response = test_app.get_admin_dashboard().await;

    assert_redirect_to(&response, "/login");
}

#[tokio::test]
pub async fn change_password_landing_page_redirects_to_login_page_given_invalid_session() {
    let test_app = spawn_app().await;

    let response = test_app.get_change_password().await;

    assert_redirect_to(&response, "/login");
}

#[tokio::test]
pub async fn change_password_request_redirects_to_login_page_given_invalid_session() {
    let test_app = spawn_app().await;

    let new_password = "12345";
    let valid_confirmation = "12345";

    let response = test_app
        .post_change_password(new_password, valid_confirmation)
        .await;

    assert_redirect_to(&response, "/login");
}

#[tokio::test]
pub async fn change_password_request_redirects_to_change_password_page_with_flash_message_on_unexpected_error()
 {
    let test_app = spawn_app().await;

    let new_password = "12345";
    let valid_confirmation = "12345";

    let valid_credentials = Credentials {
        username: test_app.test_user.username.clone(),
        password: SecretString::from(test_app.test_user.password.clone()),
    };

    test_app.post_login(&valid_credentials).await;

    sqlx::query!("ALTER TABLE users DROP COLUMN password_hash;")
        .execute(&test_app.connection_pool)
        .await
        .expect("Failed to drop column");

    let response = test_app
        .post_change_password(new_password, valid_confirmation)
        .await;

    assert_redirect_to(&response, "/admin/password");

    let response_html = test_app.get_change_password_html().await;
    assert!(response_html.contains("An unexpected error occured. Please try again."))
}

#[tokio::test]
pub async fn invalid_change_password_request_redirects_to_change_password_page_with_flash_message()
{
    let test_app = spawn_app().await;

    let new_password = "12345";
    let invalid_confirmation = "1234";

    let valid_credentials = Credentials {
        username: test_app.test_user.username.clone(),
        password: SecretString::from(test_app.test_user.password.clone()),
    };

    test_app.post_login(&valid_credentials).await;

    let response = test_app
        .post_change_password(new_password, invalid_confirmation)
        .await;

    assert_redirect_to(&response, "/admin/password");

    let response_html = test_app.get_change_password_html().await;
    assert!(response_html.contains("Passwords must match. Please try again."))
}

#[tokio::test]
pub async fn change_password_request_redirects_to_admin_page_given_valid_confirmation() {
    let test_app = spawn_app().await;

    let new_password = "12345";
    let valid_confirmation = "12345";

    let valid_credentials = Credentials {
        username: test_app.test_user.username.clone(),
        password: SecretString::from(test_app.test_user.password.clone()),
    };

    test_app.post_login(&valid_credentials).await;

    let response = test_app
        .post_change_password(new_password, valid_confirmation)
        .await;

    assert_redirect_to(&response, "/admin/dashboard");
}

#[tokio::test]
pub async fn change_password_request_does_so_given_valid_confirmation() {
    let test_app = spawn_app().await;

    let user_id = test_app.test_user.user_id;
    let new_password = "12345";
    let valid_confirmation = "12345";

    let valid_credentials = Credentials {
        username: test_app.test_user.username.clone(),
        password: SecretString::from(test_app.test_user.password.clone()),
    };

    test_app.post_login(&valid_credentials).await;

    test_app
        .post_change_password(new_password, valid_confirmation)
        .await;

    let valid_credentials = Credentials {
        username: test_app.test_user.username,
        password: SecretString::from(new_password),
    };
    let result = validate_credentials(valid_credentials, &test_app.connection_pool)
        .await
        .expect("Failed validation of credentials");

    assert_eq!(user_id, result);
}

#[tokio::test]
pub async fn dashboard_returns_success_given_valid_session() {
    let test_app = spawn_app().await;

    let valid_credentials = Credentials {
        username: test_app.test_user.username.clone(),
        password: SecretString::from(test_app.test_user.password.clone()),
    };

    test_app.post_login(&valid_credentials).await;

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

    test_app.post_login(&valid_credentials).await;

    let body = test_app.get_admin_dashboard().await.text().await.unwrap();

    assert!(body.contains(&test_app.test_user.username));
}
