use actix_web::{HttpResponse, http::header::ContentType, web};
use anyhow::Context;
use hmac::{Hmac, Mac, digest::MacError};
use secrecy::ExposeSecret;
use serde::Deserialize;

use crate::{routes::error_chain_fmt, startup::HmacSecret};

#[derive(thiserror::Error)]
pub enum QueryError {
    #[error("Tag and query string do not match")]
    MismatchError(#[from] MacError),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

#[derive(Deserialize)]
pub struct ErrorQuery {
    error: String,
    tag: String,
}
//
// make this a method for QueryParams
impl ErrorQuery {
    fn generate_query_string(error_string: &str) -> String {
        format!("error={}", urlencoding::encode(error_string))
    }

    pub fn from_error(error_string: String, hmac_secret: &HmacSecret) -> Result<Self, QueryError> {
        let query_string = Self::generate_query_string(&error_string);
        let secret = hmac_secret.0.expose_secret().as_bytes();
        let mut hmac_token =
            Hmac::<sha2::Sha256>::new_from_slice(secret).context("Invalid secret for hmac")?;
        hmac_token.update(query_string.as_bytes());
        let hmac_token = hmac_token.finalize();
        Ok(Self {
            error: error_string,
            tag: format!("{:x}", &hmac_token.into_bytes()),
        })
    }

    pub fn read_pair(error_string: &str, tag_as_hex: &str) -> Self {
        Self {
            error: error_string.into(),
            tag: tag_as_hex.into(),
        }
    }

    pub fn validate(&self, hmac_secret: &HmacSecret) -> Result<(), QueryError> {
        let tag = hex::decode(&self.tag).context("tag was invalid hex")?;
        let query_string = Self::generate_query_string(&self.error);
        let secret = hmac_secret.0.expose_secret().as_bytes();
        let mut hmac_token =
            Hmac::<sha2::Sha256>::new_from_slice(secret).context("Invalid secret for HMAC")?;
        hmac_token.update(query_string.as_bytes());
        hmac_token
            .verify_slice(&tag)
            .map_err(QueryError::MismatchError)
    }

    pub fn as_string(&self) -> String {
        let query_string = Self::generate_query_string(&self.error);
        format!("{}&tag={}", query_string, self.tag)
    }
}

pub async fn login_form(
    query: Option<web::Query<ErrorQuery>>,
    hmac_secret: web::Data<HmacSecret>,
) -> HttpResponse {
    let error_html: String = match &query {
        Some(qp) => {
            match ErrorQuery::read_pair(&qp.error, &qp.tag).validate(hmac_secret.as_ref()) {
                Ok(_) => {
                    let escaped_error = htmlescape::encode_minimal(&qp.error);
                    format!(r#"<p><i>{escaped_error}</i></p>"#)
                }
                Err(e) => {
                    tracing::warn!(
                    error.message= %e,
                    error.cause_chain = ?e,
                    "Failed to validate the query parameters using the HMAC tag"
                    );
                    "".into()
                }
            }
        }
        None => "".into(),
    };
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

        "#,
        ))
}
