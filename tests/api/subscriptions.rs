use crate::helpers::spawn_app;

#[tokio::test]
async fn subscription_returns_400_for_invalid_form_data() {
    let test_app = spawn_app().await;

    let test_cases = vec![
        ("name=jack%20cordery", "missing email"),
        ("email=jack%40gmail.com", "missing name"),
        ("", "missing both email and name"),
    ];

    for (invalid_body, test_name) in test_cases {
        let response = test_app.post_subsription(invalid_body.into()).await;

        assert_eq!(
            400,
            response.status().as_u16(),
            "The api failed to deliver a status code of 400 whilst testing {}",
            test_name
        );
    }
}

#[tokio::test]
async fn subscribe_returns_200_for_valid_form_data() {
    let test_app = spawn_app().await;

    let response = test_app
        .post_subsription("name=jack%20cordery&email=jack%40gmail.com".into())
        .await;

    assert_eq!(200, response.status().as_u16());

    let saved = sqlx::query!("SELECT email, name FROM subscriptions",)
        .fetch_one(&test_app.connection_pool)
        .await
        .expect("Failed to query connection");

    assert_eq!(saved.email, "jack@gmail.com");
    assert_eq!(saved.name, "jack cordery");
}

#[tokio::test]
async fn subscribe_returns_400_for_invalid_or_missing_data() {
    let test_app = spawn_app().await;

    let test_tupes = [
        ("name=&email=some@email.com", "missing name"),
        ("name=some%20name&email=", "missing email"),
        ("name=some%20name&email=notanemail", "invalid email"),
    ];

    for (test_input, test_name) in test_tupes {
        let response = test_app.post_subsription(test_input.into()).await;

        assert_eq!(
            400,
            response.status().as_u16(),
            "The api failed to return 400 when testing {test_name}"
        );

        let saved = sqlx::query!("SELECT email, name FROM subscriptions",)
            .fetch_one(&test_app.connection_pool)
            .await;

        assert!(
            matches!(saved, Err(sqlx::Error::RowNotFound)),
            "The api failed by returning rows that should not have been writtten to DB in test: {test_name}"
        );
    }
}
