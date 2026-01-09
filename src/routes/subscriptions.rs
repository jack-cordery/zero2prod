use actix_web::{HttpResponse, web};
use chrono::Utc;
use sqlx::PgPool;
use tracing::{self, instrument};
use uuid::Uuid;

use crate::domain::{NewSubcsriber, SubscriberEmail, SubscriberName};

#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}

#[instrument(name = "Adding a new subscriber", skip(form, connection), fields(subscriber_email = %form.email, subscriber_name = %form.name))]
pub async fn subscribe(form: web::Form<FormData>, connection: web::Data<PgPool>) -> HttpResponse {
    let name = match SubscriberName::parse(form.0.name) {
        Ok(name) => name,
        Err(_) => return HttpResponse::BadRequest().finish(),
    };
    let email = match SubscriberEmail::parse(form.0.email) {
        Ok(email) => email,
        Err(_) => return HttpResponse::BadRequest().finish(),
    };

    let new_subscriber = NewSubcsriber { name, email };
    match insert_subscriber(&connection, &new_subscriber).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

#[instrument(
    name = "Inserting new subcriber to DB",
    skip(connection, new_subsriber)
)]
pub async fn insert_subscriber(
    connection: &PgPool,
    new_subsriber: &NewSubcsriber,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
INSERT INTO subscriptions (id, email, name, subscribed_at) VALUES ($1, $2, $3, $4)
"#,
        Uuid::new_v4(),
        new_subsriber.email.as_ref(),
        new_subsriber.name.as_ref(),
        Utc::now()
    )
    .execute(connection)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;
    Ok(())
}
