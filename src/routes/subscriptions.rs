use actix_web::{HttpResponse, web};
use chrono::Utc;
use sqlx::PgPool;
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
    log::info!(
        "request_id {} - Adding new subscriber {} {}",
        request_id,
        form.name,
        form.email
    );

    log::info!("request_id {} - Saving subscriber to the DB", request_id);
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
    .await
    {
        Ok(_) => {
            log::info!(
                "request_id {} - New subscription successfully added!",
                request_id
            );
            HttpResponse::Ok().finish()
        }
        Err(e) => {
            log::error!("request_id {} - Failed to execute query: {e:?}", request_id);
            HttpResponse::InternalServerError().finish()
        }
    }
}
