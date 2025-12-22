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
    log::info!("Adding new subscriber {} {}", form.name, form.email);

    log::info!("Saving subscriber to the DB");
    match sqlx::query!(
        r#"
INSERT INTO subscriptions (id, email, name, subscribed_at) VALUES ($1, $2, $3, $4)
"#,
        Uuid::new_v4(),
        form.email,
        form.name,
        Utc::now()
    )
    .execute(connection.get_ref())
    .await
    {
        Ok(_) => {
            log::info!("New subscription successfully added!");
            HttpResponse::Ok().finish()
        }
        Err(e) => {
            log::error!("Failed to execute query: {e:?}");
            HttpResponse::InternalServerError().finish()
        }
    }
}
