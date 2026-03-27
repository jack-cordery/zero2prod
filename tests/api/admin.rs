use serde_json::json;

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

    let body = json!({
     "new_password" : "12345",
     "valid_confirmation" :"12345",
    });

    let response = test_app.post_change_password(&body).await;

    assert_redirect_to(&response, "/login");
}

#[tokio::test]
pub async fn incorrect_current_password_request_redirects_to_change_password_page_with_flash_message()
 {
    let test_app = spawn_app().await;

    let incorrect_current_password = "";
    let new_password = "12345";
    let confirmation = "12345";

    let change_password_body = json!({
        "current_password": incorrect_current_password,
        "new_password": new_password,
        "confirmation": confirmation,
    });

    let login_body = json!( {
        "username": test_app.test_user.username.clone(),
        "password": test_app.test_user.password.clone(),
    });

    test_app.post_login(&login_body).await;

    let response = test_app.post_change_password(&change_password_body).await;

    assert_redirect_to(&response, "/admin/password");

    let response_html = test_app.get_change_password_html().await;
    assert!(response_html.contains("Invalid credentials"))
}

#[tokio::test]
pub async fn mismatched_new_password_request_redirects_to_change_password_page_with_flash_message()
{
    let test_app = spawn_app().await;

    let new_password = "12345";
    let invalid_confirmation = "1234";

    let change_password_body = json!({
        "current_password": test_app.test_user.password.clone(),
        "new_password": new_password,
        "confirmation": invalid_confirmation,
    });

    let login_body = json!( {
        "username": test_app.test_user.username.clone(),
        "password": test_app.test_user.password.clone(),
    });

    test_app.post_login(&login_body).await;

    let response = test_app.post_change_password(&change_password_body).await;

    assert_redirect_to(&response, "/admin/password");

    let response_html = test_app.get_change_password_html().await;
    assert!(response_html.contains("Passwords must match. Please try again."))
}

#[tokio::test]
pub async fn change_password_request_delivers_flash_message_on_success() {
    let test_app = spawn_app().await;

    let new_password = "12345";
    let valid_confirmation = "12345";

    let change_password_body = json!({
        "current_password": test_app.test_user.password.clone(),
        "new_password": new_password,
        "confirmation": valid_confirmation,
    });

    let login_body = json!( {
        "username": test_app.test_user.username.clone(),
        "password": test_app.test_user.password.clone(),
    });

    test_app.post_login(&login_body).await;

    let change_password_response = test_app.post_change_password(&change_password_body).await;

    assert_redirect_to(&change_password_response, "/admin/dashboard");

    let html = test_app.get_dashboard_html().await;

    assert!(html.contains("Password successfully changed"))
}

#[tokio::test]
pub async fn change_password_request_does_so_given_valid_confirmation() {
    let test_app = spawn_app().await;

    let new_password = "12345";
    let valid_confirmation = "12345";

    let change_password_body = json!({
        "current_password": test_app.test_user.password.clone(),
        "new_password": new_password,
        "confirmation": valid_confirmation,
    });

    let login_body = json!( {
        "username": test_app.test_user.username.clone(),
        "password": test_app.test_user.password.clone(),
    });

    test_app.post_login(&login_body).await;

    let change_password_response = test_app.post_change_password(&change_password_body).await;

    assert_redirect_to(&change_password_response, "/admin/dashboard");

    test_app.post_logout().await;

    let body = json!( {
        "username": test_app.test_user.username.clone(),
        "password": new_password,
    });

    let login_response = test_app.post_login(&body).await;

    assert_redirect_to(&login_response, "/admin/dashboard");
}

#[tokio::test]
pub async fn dashboard_returns_success_given_valid_session() {
    let test_app = spawn_app().await;

    let body = json! ({
        "username": test_app.test_user.username.clone(),
        "password": test_app.test_user.password.clone(),
    });

    test_app.post_login(&body).await;

    let response = test_app.get_admin_dashboard().await;

    assert_eq!(200, response.status().as_u16());
}

#[tokio::test]
pub async fn dashboard_accesses_session_data_given_valid_session() {
    let test_app = spawn_app().await;

    let body = json!( {
        "username": test_app.test_user.username.clone(),
        "password": test_app.test_user.password.clone(),
    });

    test_app.post_login(&body).await;

    let body = test_app.get_admin_dashboard().await.text().await.unwrap();

    assert!(body.contains(&test_app.test_user.username));
}

#[tokio::test]
pub async fn logout_stops_user_from_accessing_admin_after_changing_password() {
    let test_app = spawn_app().await;

    let new_password = "12345";
    let valid_confirmation = "12345";

    let change_password_body = json!({
        "current_password": test_app.test_user.password.clone(),
        "new_password": new_password,
        "confirmation": valid_confirmation,
    });

    let login_body = json!( {
        "username": test_app.test_user.username.clone(),
        "password": test_app.test_user.password.clone(),
    });

    test_app.post_login(&login_body).await;

    test_app.post_change_password(&change_password_body).await;

    let logout_response = test_app.post_logout().await;

    assert_redirect_to(&logout_response, "/");

    let admin_response = test_app.get_admin_dashboard().await;

    assert_redirect_to(&admin_response, "/login");
}
