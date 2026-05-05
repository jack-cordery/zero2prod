use anyhow::{Context, anyhow};
use argon2::{
    Argon2, Params, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{SaltString, rand_core::OsRng},
};
use secrecy::{ExposeSecret, SecretString};
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;

use crate::{routes::error_chain_fmt, telementry::spawn_blocking_thread_with_span};

#[derive(thiserror::Error)]
pub enum AuthError {
    #[error("Invalid Credentials")]
    InvalidCredentialsError(#[source] anyhow::Error),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

pub struct Credentials {
    pub username: String,
    pub password: SecretString,
}

#[tracing::instrument(name = "Validate credentials", skip(credentials, pool))]
pub async fn validate_credentials(
    credentials: Credentials,
    pool: &PgPool,
) -> Result<Uuid, AuthError> {
    let mut user_id: Option<Uuid> = None;

    let mut expected_password_hash = SecretString::new(
        "$argon2id$v=19$m=15000,t=2,p=1$\
gZiV/M1gPc22ElAH/Jh1Hw$\
CWOrkoo7oJBQ/iyh7uJ0LO2aLEfrHwTWllSAxT0zRno"
            .into(),
    );

    if let Some((stored_user_id, stored_expected_password_hash)) =
        get_strored_credentials(&credentials.username, pool)
            .await
            .map_err(AuthError::UnexpectedError)?
    {
        user_id = Some(stored_user_id);
        expected_password_hash = stored_expected_password_hash;
    };

    spawn_blocking_thread_with_span(move || {
        verify_password(expected_password_hash, credentials.password)
    })
    .await
    .context("Spawning blocking thread failed.")
    .map_err(AuthError::UnexpectedError)??;

    user_id.ok_or(AuthError::InvalidCredentialsError(anyhow!(
        "Unknown username."
    )))
}

#[tracing::instrument(name = "Get stored credentials", skip(username, pool))]
pub async fn get_strored_credentials(
    username: &str,
    pool: &PgPool,
) -> Result<Option<(Uuid, SecretString)>, anyhow::Error> {
    let optional_row = sqlx::query!(
        r#"SELECT user_id, password_hash FROM users WHERE username = $1;"#,
        username
    )
    .fetch_optional(pool)
    .await
    .context("Failed to perform a query to retrieve stored credentials")?;

    Ok(optional_row.map(|row| (row.user_id, SecretString::from(row.password_hash))))
}

#[tracing::instrument(
    name = "Verifying password",
    skip(expected_password_hash, password_candidate)
)]
pub fn verify_password(
    expected_password_hash: SecretString,
    password_candidate: SecretString,
) -> Result<(), AuthError> {
    let expected_password_hash = PasswordHash::new(expected_password_hash.expose_secret())
        .context("Failed to parse hashed password from PHC string")
        .map_err(AuthError::UnexpectedError)?;
    tracing::info_span!("Verify password hash")
        .in_scope(|| {
            Argon2::default().verify_password(
                password_candidate.expose_secret().as_bytes(),
                &expected_password_hash,
            )
        })
        .context("Failed to verigy password")
        .map_err(AuthError::InvalidCredentialsError)
}

pub fn generate_password_hash(password: SecretString) -> String {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        Params::new(15000, 2, 1, None).expect("Failed to initialise Argon2 params"),
    )
    .hash_password(password.expose_secret().as_bytes(), &salt)
    .expect("Failed to hash password")
    .to_string()
}

#[tracing::instrument(name = "Updating password in database", skip(new_password, pool))]
pub async fn update_password(
    user_id: Uuid,
    new_password: SecretString,
    pool: &PgPool,
) -> Result<(), AuthError> {
    let password_hash = generate_password_hash(new_password);
    sqlx::query!(
        r#"
            UPDATE users
            SET password_hash=$2
            WHERE user_id=$1; 
"#,
        user_id,
        password_hash
    )
    .execute(pool)
    .await
    .context("Failed to update password in database")
    .map_err(AuthError::UnexpectedError)?;
    Ok(())
}

#[instrument(name = "Get username from user_id", skip(pool))]
pub async fn get_user(user_id: Uuid, pool: &PgPool) -> Result<String, anyhow::Error> {
    let username = sqlx::query!("SELECT username FROM users WHERE user_id=$1", user_id)
        .fetch_one(pool)
        .await
        .context("Failed to query database")?
        .username;
    Ok(username)
}

#[instrument(name = "Get user_id from email", skip(pool))]
pub async fn authorize_email(email: String, pool: &PgPool) -> Result<Uuid, AuthError> {
    let user_id = sqlx::query!("SELECT user_id FROM users WHERE username=$1", email)
        .fetch_one(pool)
        .await
        .context("Failed to query database")
        .map_err(AuthError::InvalidCredentialsError)?
        .user_id;
    Ok(user_id)
}
