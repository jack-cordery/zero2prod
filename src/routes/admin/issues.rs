use actix_web::{HttpResponse, web};
use actix_web_flash_messages::IncomingFlashMessages;
use sqlx::PgPool;
use std::fmt::Write;
use tracing::instrument;

use crate::utils::e500;

#[instrument(name = "handling get issues", skip(pool, messages))]
pub async fn issues(
    pool: web::Data<PgPool>,
    messages: IncomingFlashMessages,
) -> Result<HttpResponse, actix_web::Error> {
    let pool = pool.as_ref();
    let mut flash_messages = String::from("");
    for message in messages.iter() {
        let message_str = message.content();
        writeln!(flash_messages, "<p><i>{message_str}</i></p>").expect("Unable to write to buffer");
    }
    let record = sqlx::query!(
        r#"
    SELECT n.id, u.username, n.title, COUNT(e.newsletter_id)
    FROM newsletters n
    LEFT JOIN users u ON n.user_id = u.user_id 
    LEFT JOIN email_processing_tasks e ON n.id = e.newsletter_id
    GROUP BY
    n.id, u.username, n.title
    ;
    "#
    )
    .fetch_all(pool)
    .await
    .map_err(e500)?;

    let html_issue_list = match record.len() {
        0 => "No issues found.".into(),
        _ => {
            let mut buffer = String::from("");
            for item in record.iter() {
                let issue_id = item.id.to_string();
                let (_, last_issue_id) = issue_id.split_at(issue_id.len() - 4);
                let item_string = match item.count {
                    Some(0) | None => format!(
                        "Issue ID: {}, Username: {}, Title: {}, Status: Completed",
                        last_issue_id, item.username, item.title
                    ),
                    Some(n) => format!(
                        "Issue ID: {}, User ID: {}, Title: {}, Status: In progress, Tasks left: {}",
                        item.id, item.username, item.title, n
                    ),
                };
                writeln!(buffer, "<li>{}</li>", item_string)
                    .expect("Unable to write item into buffer");
            }
            buffer
        }
    };

    let html = format!(
        r#"
    <!DOCTYPE html>
    <html lang="en">
        <title> Issues </title>
        <body>
        <h1>Published Issues</h1>
        {flash_messages}
        <a href="/admin/issues">
            <button> 
            Refresh
            </button>
        </a>
        {html_issue_list}
        </body>
    </html>
    "#
    );
    Ok(HttpResponse::Ok().body(html))
}
