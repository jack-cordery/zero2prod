use std::fmt::Debug;
use std::fmt::Display;

use std::fmt::Write;

use actix_web::HttpResponse;
use actix_web::error::ErrorInternalServerError;
use actix_web_flash_messages::IncomingFlashMessages;
use actix_web_flash_messages::Level;
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

#[instrument(
    name = "Handling GET admin/dashboard",
    skip(pool, session, flash_messages)
)]
pub async fn dashboard(
    pool: actix_web::web::Data<PgPool>,
    session: TypedSession,
    flash_messages: IncomingFlashMessages,
) -> Result<HttpResponse, actix_web::Error> {
    let mut messages: String = "".into();

    for message in flash_messages.iter().filter(|m| m.level() == Level::Info) {
        writeln!(messages, "<p><i>{}</i></p>", message.content())
            .expect("Failed to write message into buffer");
    }
    let user_id = session
        .get_user_id()
        .context("Failed to retrieve session from valkey")
        .map_err(e500)?
        .expect("User not found even after middleware validation");

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
                {}
                <p>Welcome {username}!</p>
                <a href="/admin/password">
                    <button>
                        Change password
                    </button>
                </a>
                </body>
                </html>"#,
        messages
    );
    return Ok(HttpResponse::Ok().body(body));
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
