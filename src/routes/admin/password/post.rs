use std::fmt::{Debug, Display};

use actix_web::{
    HttpResponse,
    error::{ErrorInternalServerError, InternalError},
    web,
};
use actix_web_flash_messages::FlashMessage;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;

use anyhow::{Context, anyhow};
use sqlx::PgPool;
use tracing::{field::Empty, instrument};

use crate::{
    authentication::{AuthError, update_password},
    routes::error_chain_fmt,
    session_state::TypedSession,
};

#[derive(Deserialize)]
pub struct NewPassword {
    password: SecretString,
    confirmation: SecretString,
}

impl NewPassword {
    pub fn verify(&self) -> Result<(), PasswordError> {
        if self.password.expose_secret() == self.confirmation.expose_secret() {
            Ok(())
        } else {
            Err(PasswordError::PasswordMismatchError(anyhow!(
                "password and confirmation don't match"
            )))
        }
    }
}

#[instrument(name="Handling change password", skip(form, session, pool), fields(user_id=Empty))]
pub async fn change_password(
    form: web::Form<NewPassword>,
    session: TypedSession,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, actix_web::Error> {
    form.0.verify().map_err(unexpected_redirect)?;

    let user_id = session
        .get_user_id()
        .map_err(e500)?
        .context("user_id not available from session")
        .map_err(|e| unexpected_redirect(PasswordError::UnexpectedError(e)))?;

    tracing::Span::current().record("user_id", user_id.to_string());

    update_password(user_id, form.0.password, &pool)
        .await
        .map_err(|e| match e {
            AuthError::UnexpectedError(e) => unexpected_redirect(PasswordError::UnexpectedError(e)),
            AuthError::InvalidCredentialsError(e) => {
                auth_redirect(PasswordError::InvalidCredentials(e))
            }
        })?;

    FlashMessage::info("Password successfully changed").send();
    let response = HttpResponse::SeeOther()
        .insert_header((actix_web::http::header::LOCATION, "/admin/dashboard"))
        .finish();
    Ok(response)
}

pub fn auth_redirect(e: PasswordError) -> InternalError<PasswordError> {
    FlashMessage::error(e.to_string()).send();
    let response = HttpResponse::SeeOther()
        .insert_header((actix_web::http::header::LOCATION, "/login"))
        .finish();
    InternalError::from_response(e, response)
}

pub fn unexpected_redirect(e: PasswordError) -> InternalError<PasswordError> {
    FlashMessage::error(e.to_string()).send();
    let response = HttpResponse::SeeOther()
        .insert_header((actix_web::http::header::LOCATION, "/admin/password"))
        .finish();
    InternalError::from_response(e, response)
}

#[derive(thiserror::Error)]
pub enum PasswordError {
    #[error("An unexpected error occured. Please try again.")]
    UnexpectedError(anyhow::Error),
    #[error("Passwords must match. Please try again.")]
    PasswordMismatchError(anyhow::Error),
    #[error("Invalid credentials")]
    InvalidCredentials(anyhow::Error),
}

impl std::fmt::Debug for PasswordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

fn e500<T>(error: T) -> actix_web::Error
where
    T: Debug + Display + 'static,
{
    ErrorInternalServerError(error)
}
