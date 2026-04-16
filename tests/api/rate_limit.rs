use zero2prod::configuration::RateLimitSettings;

use crate::helpers::spawn_app_with_config;

#[tokio::test]
pub async fn server_returns_429_when_rate_limit_is_breached() {
    let rate_limit = RateLimitSettings {
        rate_limit: 10,
        namespace: "rate-limit-test".into(),
    };
    let test_app = spawn_app_with_config(rate_limit).await;
    for _ in 0..10 {
        let response = test_app.get_admin_dashboard().await;
        assert_ne!(429, response.status().as_u16());
    }
    let response = test_app.get_admin_dashboard().await;
    assert_eq!(429, response.status().as_u16());
}
