use actix_web::{
    HttpResponse,
    error::InternalError,
    web::{self, ReqData},
};
use actix_web_flash_messages::FlashMessage;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;

use anyhow::anyhow;
use sqlx::PgPool;
use tracing::instrument;

use crate::authentication::{
    AuthError, Credentials, UserId, get_user, update_password, validate_credentials,
};
use crate::routes::error_chain_fmt;

#[derive(Deserialize)]
pub struct NewPasswordForm {
    current_password: SecretString,
    new_password: SecretString,
    confirmation: SecretString,
}

impl NewPasswordForm {
    pub async fn validate(&self, username: &str, pool: &PgPool) -> Result<(), PasswordError> {
        if self.new_password.expose_secret() == self.confirmation.expose_secret() {
            match validate_credentials(
                Credentials {
                    username: username.to_string(),
                    password: self.current_password.clone(),
                },
                pool,
            )
            .await
            {
                Ok(_) => Ok(()),
                Err(e) => Err(PasswordError::InvalidCredentials(anyhow!(e.to_string()))),
            }
        } else {
            Err(PasswordError::PasswordMismatchError(anyhow!(
                "password and confirmation don't match"
            )))
        }
    }
}

#[instrument(name = "Handling change password", skip(form, pool))]
pub async fn change_password(
    form: web::Form<NewPasswordForm>,
    user_id: ReqData<UserId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = user_id.into_inner();
    let username = get_user(*user_id, &pool)
        .await
        .map_err(|e| error_redirect(PasswordError::UnexpectedError(e)))?;

    form.0
        .validate(&username, &pool)
        .await
        .map_err(error_redirect)?;

    update_password(*user_id, form.0.new_password, &pool)
        .await
        .map_err(|e| match e {
            AuthError::UnexpectedError(e) => error_redirect(PasswordError::UnexpectedError(e)),
            AuthError::InvalidCredentialsError(e) => {
                error_redirect(PasswordError::InvalidCredentials(e))
            }
        })?;

    FlashMessage::info("Password successfully changed").send();
    let response = HttpResponse::SeeOther()
        .insert_header((actix_web::http::header::LOCATION, "/admin/dashboard"))
        .finish();
    Ok(response)
}

pub fn error_redirect(e: PasswordError) -> InternalError<PasswordError> {
    FlashMessage::error(e.to_string()).send();
    let response = HttpResponse::SeeOther()
        .insert_header((actix_web::http::header::LOCATION, "/admin/password"))
        .finish();

    InternalError::from_response(e, response)
}

#[derive(thiserror::Error)]
pub enum PasswordError {
    #[error("An unexpected error occured. Please try again.")]
    UnexpectedError(#[from] anyhow::Error),
    #[error("Passwords must match. Please try again.")]
    PasswordMismatchError(#[source] anyhow::Error),
    #[error("Invalid credentials")]
    InvalidCredentials(#[source] anyhow::Error),
}

impl std::fmt::Debug for PasswordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}
