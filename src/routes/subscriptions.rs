use actix_web::{HttpResponse, web};
use chrono::Utc;
use sqlx::PgPool;
use tracing::{self, instrument};
use uuid::Uuid;

use crate::domain::{NewSubscriber, SubscriberEmail, SubscriberName, SubscriberStatus};

impl TryFrom<FormData> for NewSubscriber {
    type Error = String;

    fn try_from(form: FormData) -> Result<Self, Self::Error> {
        let name = SubscriberName::parse(form.name)?;
        let email = SubscriberEmail::parse(form.email)?;
        let status = SubscriberStatus::Pending;

        let new_subscriber = NewSubscriber {
            name,
            email,
            status,
        };
        Ok(new_subscriber)
    }
}

#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}

#[instrument(name = "Adding a new subscriber", skip(form, connection), fields(subscriber_email = %form.email, subscriber_name = %form.name))]
pub async fn subscribe(form: web::Form<FormData>, connection: web::Data<PgPool>) -> HttpResponse {
    let new_subscriber: NewSubscriber = match form.0.try_into() {
        Ok(new_subcriber) => new_subcriber,
        Err(_) => return HttpResponse::BadRequest().finish(),
    };
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
    new_subsriber: &NewSubscriber,
) -> Result<(), sqlx::Error> {
    let initial_status: String = new_subsriber.status.into();
    sqlx::query!(
        r#"
INSERT INTO subscriptions (id, email, name, subscribed_at, status) VALUES ($1, $2, $3, $4, $5)
"#,
        Uuid::new_v4(),
        new_subsriber.email.as_ref(),
        new_subsriber.name.as_ref(),
        Utc::now(),
        initial_status,
    )
    .execute(connection)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;
    Ok(())
}
