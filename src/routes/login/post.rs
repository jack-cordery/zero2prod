use std::fmt;

use actix_web::{HttpResponse, error::InternalError, http::StatusCode};
use secrecy::SecretString;
use serde::Deserialize;
use sqlx::PgPool;
use tracing::{field::Empty, instrument};

use crate::{
    authentication::{AuthError, Credentials, validate_credentials},
    routes::{error_chain_fmt, login::get::ErrorQuery},
    startup::HmacSecret,
};

#[derive(Deserialize)]
pub struct LoginForm {
    username: String,
    password: SecretString,
}

impl From<LoginForm> for Credentials {
    fn from(form: LoginForm) -> Self {
        Credentials {
            username: form.username,
            password: form.password,
        }
    }
}

#[instrument(name="Handling post login request", skip(form, pool, hmac_secret), fields(username=Empty, user_id=Empty))]
pub async fn login(
    form: actix_web::web::Form<LoginForm>,
    pool: actix_web::web::Data<PgPool>,
    hmac_secret: actix_web::web::Data<HmacSecret>,
) -> Result<HttpResponse, InternalError<LoginError>> {
    tracing::Span::current().record("username", tracing::field::display(&form.0.username));
    match validate_credentials(form.0.into(), &pool).await {
        Ok(user_id) => {
            tracing::Span::current().record("user_id", tracing::field::display(user_id));
            Ok(HttpResponse::SeeOther()
                .insert_header((actix_web::http::header::LOCATION, "/"))
                .finish())
        }
        Err(e) => {
            let e = match e {
                AuthError::InvalidCredentialsError(e) => LoginError::AuthError(e),
                AuthError::UnexpectedError(e) => LoginError::UnexpectedError(e),
            };
            let query =
                ErrorQuery::from_error(e.to_string(), &hmac_secret).expect("Invalid error string");

            Err(InternalError::from_response(
                e,
                HttpResponse::build(StatusCode::SEE_OTHER)
                    .insert_header((
                        actix_web::http::header::LOCATION,
                        format!("/login?{}", query.as_string()),
                    ))
                    .finish(),
            ))
        }
    }
}

#[derive(thiserror::Error)]
pub enum LoginError {
    #[error("Authentication Failed")]
    AuthError(anyhow::Error),
    #[error("Something went wrong")]
    UnexpectedError(anyhow::Error),
}

impl std::fmt::Debug for LoginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        error_chain_fmt(self, f)
    }
}
