use actix_web::{HttpResponse, http::header::ContentType};
use actix_web_flash_messages::{IncomingFlashMessages, Level};
use std::fmt::Write;
use tracing::instrument;

#[instrument(name = "serving password form", skip(flash_messages))]
pub async fn password_form(flash_messages: IncomingFlashMessages) -> HttpResponse {
    let mut error_html = String::from("");
    for message in flash_messages.iter().filter(|m| m.level() == Level::Error) {
        writeln!(error_html, "<p><i>{}<p><i>", message.content())
            .expect("Failed to load info string");
    }
    let body = format!(
        r#"
<!DOCTYPE HTML>
	<html lang="en">
		<head>
			<title>
			    Change password
			</title>
		</head>
		<body>
                        {error_html}
			<p>
			    Change password bellow
			</p>
			<form action="/admin/password" method="post">
    				<label>
					Current
					<input  type="password" placeholder="New Password" name="current_password">
					</input>
    				</label>
    				<label>
					New password
					<input  type="password" placeholder="New Password" name="new_password">
					</input>
    				</label>
    				<label>
					Confirm password
					<input  type="password" placeholder="Confirm Password" name="confirmation">
					</input>
    				</label>
				<button type="submit">
				Confirm
				</button>

			</form>
		</body>
	</html>
    "#
    );
    HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(body)
}
