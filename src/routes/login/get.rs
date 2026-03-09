use actix_web::{HttpResponse, http::header::ContentType};
use actix_web_flash_messages::{IncomingFlashMessages, Level};
use std::fmt::Write;
use tracing::instrument;

#[instrument(name = "Handling get login request", skip(flash_messages))]
pub async fn login_form(flash_messages: IncomingFlashMessages) -> HttpResponse {
    let mut error_html = String::new();
    for message in flash_messages
        .iter()
        .filter(|message| message.level() == Level::Error)
    {
        writeln!(error_html, "<p><i>{}</i><p>", message.content())
            .expect("Failed to load error string");
    }
    HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(format!(
            r#"
<!DOCTYPE html> 
<html lang="en">
	<head>
		Login
	</head>
        <body>
        {error_html}
	<form action="/login" method="post">
		<label>
			Username
			<input
				type="text"
				placeholder= "Username"
				name="username"
			>
			</input>
		</label>
		<label>
			Password
			<input
				type="password"
				placeholder= "Password"
				name="password"
			>
			</input>
		</label>
		<button type="submit">
			Login
		</button>
	</form>
        </body>
</html>

        "#
        ))
}
