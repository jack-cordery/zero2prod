use actix_web::{HttpResponse, web};
use chrono::Utc;
use sqlx::PgPool;
use tracing::{self, Instrument, Level, info_span};
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}
pub async fn subscriptions(
    form: web::Form<FormData>,
    connection: web::Data<PgPool>,
) -> HttpResponse {
    let request_id = Uuid::new_v4();
    let request_span = info_span!("Adding a new subcriber.",
        %request_id,
        subscriber_email = %form.email,
        subscriber_name = %form.name,
    );

    let _request_span_guard = request_span.enter(); // Shouldnt be done in async TODO: Change! 
    tracing::info!(
        "request_id {} - Adding new subscriber {} {}",
        request_id,
        form.name,
        form.email
    );

    let query_span = tracing::info_span!("Saving subscriber to the DB");

    match sqlx::query!(
        r#"
INSERT INTO subscriptions (id, email, name, subscribed_at) VALUES ($1, $2, $3, $4)
"#,
        request_id,
        form.email,
        form.name,
        Utc::now()
    )
    .execute(connection.get_ref())
    .instrument(query_span)
    .await
    {
        Ok(_) => {
            tracing::info!(
                "request_id {} - New subscription successfully added!",
                request_id
            );
            HttpResponse::Ok().finish()
        }
        Err(e) => {
            tracing::error!("request_id {} - Failed to execute query: {e:?}", request_id);
            HttpResponse::InternalServerError().finish()
        }
    }
}
