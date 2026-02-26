use actix_web::{
    HttpRequest, HttpResponse, ResponseError,
    body::BoxBody,
    http::header::{HeaderMap, WWW_AUTHENTICATE},
    web,
};
use anyhow::{Context, anyhow};
use base64::{Engine, prelude::BASE64_STANDARD};
use secrecy::{ExposeSecret, SecretString};
use sha3::Digest;
use sqlx::PgPool;

use crate::{
    domain::{SubscriberEmail, SubscriberStatus},
    email_client::EmailClient,
    routes::error_chain_fmt,
};

#[derive(serde::Deserialize)]
pub struct BodyData {
    title: String,
    content: Content,
}
#[derive(serde::Deserialize)]
pub struct Content {
    text: String,
    html: String,
}

pub struct ConfirmedSubscriber {
    email: SubscriberEmail,
}

#[derive(thiserror::Error)]
pub enum PublishError {
    #[error("Authentication failed.")]
    AuthError(#[source] anyhow::Error),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for PublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for PublishError {
    fn error_response(&self) -> HttpResponse<BoxBody> {
        match &self {
            Self::UnexpectedError(_) => HttpResponse::InternalServerError().finish(),
            Self::AuthError(_) => HttpResponse::Unauthorized()
                .insert_header((WWW_AUTHENTICATE, r#"Basic realm="publish""#))
                .finish(),
        }
    }
}

pub struct Credentials {
    username: String,
    password: SecretString,
}

pub fn basic_authentication(headers: &HeaderMap) -> Result<Credentials, anyhow::Error> {
    let auth_header = headers
        .get("Authorization")
        .context("No authorization header provided")?
        .to_str()
        .context("The authorization header was not valid UTF-8 ASCII string")?;

    let auth_header_basic_removed = auth_header
        .strip_prefix("Basic ")
        .context("Autherization header was not Basic")?;

    let decoded_auth_header = BASE64_STANDARD
        .decode(auth_header_basic_removed)
        .context("The authorization header was not valid base64")?;
    let decoded_str =
        String::from_utf8(decoded_auth_header).context("Decoded credentails were not utf-8")?;
    let mut decoded_split = decoded_str.splitn(2, ":");
    let username = decoded_split
        .next()
        .ok_or_else(|| anyhow!("Username must be provided in 'Basic' auth."))?;

    let password = decoded_split
        .next()
        .ok_or_else(|| anyhow!("Password must be provided in 'Basic' auth."))?;

    Ok(Credentials {
        username: username.to_string(),
        password: SecretString::new(password.into()),
    })
}

pub async fn validate_credentials(
    credentials: &Credentials,
    pool: &PgPool,
) -> Result<uuid::Uuid, PublishError> {
    let password_hash = sha3::Sha3_256::digest(credentials.password.expose_secret().as_bytes());
    let password_hash = format!("{:x}", password_hash);

    let optional_row = sqlx::query!(
        "SELECT user_id FROM users WHERE username = $1 AND password_hash = $2;",
        credentials.username,
        &password_hash,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| PublishError::UnexpectedError(anyhow!(e)))?;

    let user_id = optional_row
        .map(|row| row.user_id)
        .ok_or_else(|| anyhow!("Invalid Username or password"))
        .map_err(PublishError::AuthError)?;

    Ok(user_id)
}

#[tracing::instrument(name = "Publish newsletter", skip(body, email_client), fields(newsletter_title=%body.title, username, user_id))]
pub async fn publish_newsletter(
    body: web::Json<BodyData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    request: HttpRequest,
) -> Result<HttpResponse, PublishError> {
    let credentials = basic_authentication(request.headers()).map_err(PublishError::AuthError)?;
    tracing::Span::current().record("username", &credentials.username);
    let user_id = validate_credentials(&credentials, &pool).await?;
    tracing::Span::current().record("user_id", user_id.to_string());
    let confirmed_subscriptions = get_confirmed_subscriptions(&pool).await?;
    for sub in confirmed_subscriptions {
        match sub {
            Ok(s) => {
                email_client
                    .send_email(
                        &s.email,
                        &body.title,
                        &body.content.text,
                        &body.content.html,
                    )
                    .await
                    .with_context(|| format!("Failed to send email to {}", s.email))?;
            }
            Err(e) => {
                tracing::warn!("Confirmed subscriber is using invalide email {}", e)
            }
        }
    }
    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(name = "Get confirmed subscriptions", skip(pool))]
pub async fn get_confirmed_subscriptions(
    pool: &PgPool,
) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
    // so here we want to query the db for confirmed subs
    let confirmed_status: String = SubscriberStatus::Confirmed.into();
    let confirmed_rows = sqlx::query!(
        r#"SELECT email FROM subscriptions WHERE status = $1"#,
        confirmed_status,
    )
    .fetch_all(pool)
    .await?;
    let confirmed_subscriptions = confirmed_rows
        .into_iter()
        .map(|r| {
            let sub_email = SubscriberEmail::parse(r.email);
            match sub_email {
                Ok(s) => Ok(ConfirmedSubscriber { email: s }),
                Err(error) => Err(anyhow::anyhow!(error)),
            }
        })
        .collect();
    Ok(confirmed_subscriptions)
}
