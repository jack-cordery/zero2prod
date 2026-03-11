use std::fmt::Debug;
use std::fmt::Display;

use actix_web::error::ErrorInternalServerError;
use actix_web::{HttpResponse, http::header::LOCATION};
use anyhow::Context;
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;

use crate::session_state::TypedSession;

fn e500<T>(error: T) -> actix_web::Error
where
    T: Debug + Display + 'static,
{
    ErrorInternalServerError(error)
}

#[instrument(name = "Handling GET admin/dashboard", skip(pool, session))]
pub async fn dashboard(
    pool: actix_web::web::Data<PgPool>,
    session: TypedSession,
) -> Result<HttpResponse, actix_web::Error> {
    if let Some(user_id) = session
        .get_user_id()
        .context("Failed to retrieve session from valkey")
        .map_err(e500)?
    {
        let username = get_user(user_id, &pool).await.map_err(e500)?;
        let body = format!(
            r#"
                <!DOCTYPE html>
                <html lang="en">
                <head>
                <meta http-equiv="content-type" content="text/html; charset=utf-8">
                <title>Admin dashboard</title>
                </head>
                <body>
                <p>Welcome {username}!</p>
                </body>
                </html>"#
        );
        return Ok(HttpResponse::Ok().body(body));
    } else {
        return Ok(HttpResponse::SeeOther()
            .insert_header((LOCATION, "/login"))
            .finish());
    };
}

#[instrument(name = "Get username from user_id", skip(pool))]
async fn get_user(user_id: Uuid, pool: &PgPool) -> Result<String, anyhow::Error> {
    let username = sqlx::query!("SELECT username FROM users WHERE user_id=$1", user_id)
        .fetch_one(pool)
        .await
        .context("Failed to query database")?
        .username;
    Ok(username)
}
