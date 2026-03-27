use actix_web::{HttpResponse, web::ReqData};
use actix_web_flash_messages::IncomingFlashMessages;
use std::fmt::Write;
use tracing::instrument;

use crate::authentication::UserId;

#[instrument(name = "handling get newsletter", skip(flash_messages))]
pub async fn newsletter_form(
    flash_messages: IncomingFlashMessages,
    user_id: ReqData<UserId>,
) -> HttpResponse {
    let mut message_html = String::new();
    for message in flash_messages.iter() {
        writeln!(message_html, "<p><i>{}</p></i>", message.content())
            .expect("Failed to write to buffer");
    }
    let body = format!(
        r#"
    <!DOCTYPE HTML> 
    <html lang="en">
    <head>
    Publish Newsletter
    </head>
    <body>
    <p>Please fill in the below to publish a newsletter to <b>ALL</b> subscribers.</p> 
    {message_html}
    <form action="/admin/newsletter" method="post">
    <ul>
        <li>
            <label>
            Title
                <input id="title" name="title" placeholder="Enter Title" required>
                </input>
            </label>
            </li>
        <li>
            <label>
            Html Content
                <textarea id="html" name="html" required>
                </textarea>
            </label>
        </li>
        <li>
            <label>
            Text Content
                <textarea id="text" name="text" required>
                </textarea>
            </label>
        </li>
        <li>
            <button type="submit">
            Publish
            </button>
        </li>
    <ul>
    </form>
    </body>
    </html>
    "#
    );
    HttpResponse::Ok().body(body)
}
