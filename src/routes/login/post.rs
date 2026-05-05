use actix_web::{HttpResponse, error::InternalError};
use actix_web_flash_messages::FlashMessage;
use anyhow::Context;
use secrecy::SecretString;
use serde::Deserialize;
use sqlx::PgPool;
use tracing::{field::Empty, instrument};

use crate::{
    authentication::{AuthError, Credentials, validate_credentials},
    routes::error_chain_fmt,
    session_state::TypedSession,
    utils::see_other,
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

pub const REDIRECT_SUCCESSFUL_LOGIN: &str = "/admin/dashboard";
pub const REDIRECT_FAILED_LOGIN: &str = "/login";

#[instrument(name="Handling post login request", skip(form, pool, session), fields(username=Empty, user_id=Empty))]
pub async fn login(
    form: actix_web::web::Form<LoginForm>,
    pool: actix_web::web::Data<PgPool>,
    session: TypedSession,
) -> Result<HttpResponse, InternalError<LoginError>> {
    tracing::Span::current().record("username", tracing::field::display(&form.0.username));
    match validate_credentials(form.0.into(), &pool).await {
        Ok(user_id) => {
            tracing::Span::current().record("user_id", tracing::field::display(user_id));
            session.renew();
            session
                .insert_user_id(user_id)
                .context("Failed to insert session into valkey")
                .map_err(|e| {
                    let e = LoginError::UnexpectedError(e);
                    login_redirect(e)
                })?;
            Ok(see_other(REDIRECT_SUCCESSFUL_LOGIN))
        }
        Err(e) => {
            let e = match e {
                AuthError::InvalidCredentialsError(e) => LoginError::AuthError(e),
                AuthError::UnexpectedError(e) => LoginError::UnexpectedError(e),
            };
            Err(login_redirect(e))
        }
    }
}

pub fn login_redirect(e: LoginError) -> InternalError<LoginError> {
    FlashMessage::error(e.to_string()).send();
    let response = see_other(REDIRECT_FAILED_LOGIN);
    InternalError::from_response(e, response)
}

#[derive(thiserror::Error)]
pub enum LoginError {
    #[error("Authentication failed")]
    AuthError(#[source] anyhow::Error),
    #[error("Something went wrong")]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for LoginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}
