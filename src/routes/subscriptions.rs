use actix_web::{HttpResponse, ResponseError, http::StatusCode, web};
use anyhow::Context;
use chrono::Utc;
use rand::{Rng, distr::Alphanumeric};
use sqlx::{PgPool, Postgres, Transaction};
use tracing::{self, instrument};
use uuid::Uuid;

use crate::{
    domain::{NewSubscriber, SubscriberEmail, SubscriberName, SubscriberStatus},
    email_client::EmailClient,
    startup::ApplicationBaseUrl,
};

impl TryFrom<FormData> for NewSubscriber {
    type Error = String;

    fn try_from(form: FormData) -> Result<Self, Self::Error> {
        let name = SubscriberName::parse(form.name)?;
        let email = SubscriberEmail::parse(form.email)?;
        let status = SubscriberStatus::Pending;

        let new_subscriber = NewSubscriber {
            name,
            email,
            status,
        };
        Ok(new_subscriber)
    }
}

#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}

#[instrument(name = "Adding a new subscriber", skip(form, connection, email_client, application_base_url), fields(subscriber_email = %form.email, subscriber_name = %form.name))]
pub async fn subscribe(
    form: web::Form<FormData>,
    connection: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    application_base_url: web::Data<ApplicationBaseUrl>,
) -> Result<HttpResponse, SubscribeError> {
    let new_subscriber: NewSubscriber =
        form.0.try_into().map_err(SubscribeError::ValidationError)?;
    let mut transaction = connection
        .begin()
        .await
        .context("Failed to acquire Postgres connection from the pool")?;

    let insert_response = insert_subscriber(&mut transaction, &connection, &new_subscriber).await?;

    let subscription_token = if insert_response.new {
        let subscription_token = generate_subscription_token();
        store_token(
            insert_response.subscriber_id,
            &subscription_token,
            &mut transaction,
        )
        .await
        .context("Failed to store confirmation token for a new subscriber")?;
        subscription_token
    } else {
        get_token_from_subscriber_id(&insert_response.subscriber_id, &connection)
            .await
            .context("Failed to get confirmation token from subscriber id")?
    };

    transaction
        .commit()
        .await
        .context("Failed to commit transaction to store new subscriber")?;

    send_confirmation_email(
        &email_client,
        new_subscriber,
        &application_base_url.0,
        &subscription_token,
    )
    .await
    .context("Failed to send confirmation email to new subscriber")?;

    Ok(HttpResponse::Ok().finish())
}

pub struct InsertResponse {
    subscriber_id: Uuid,
    new: bool,
}

#[instrument(
    name = "Inserting new subcriber to DB",
    skip(transaction, new_subscriber)
)]
pub async fn insert_subscriber(
    transaction: &mut Transaction<'_, Postgres>,
    pool: &PgPool,
    new_subscriber: &NewSubscriber,
) -> Result<InsertResponse, SubscribeError> {
    let initial_status: String = new_subscriber.status.into();
    let user_id = Uuid::new_v4();
    let query_result = sqlx::query!(
        r#"
INSERT INTO subscriptions (id, email, name, subscribed_at, status) VALUES ($1, $2, $3, $4, $5)
"#,
        user_id,
        new_subscriber.email.as_ref(),
        new_subscriber.name.as_ref(),
        Utc::now(),
        initial_status,
    )
    .execute(&mut **transaction)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    });
    match query_result {
        Ok(_) => {
            return Ok(InsertResponse {
                subscriber_id: user_id,
                new: true,
            });
        }
        Err(sqlx::Error::Database(db_err)) => {
            if db_err.code().as_deref() == Some("23505") {
                let subscriber_id =
                    check_duplicate_user_is_unconfirmed(&new_subscriber.email, pool).await?;
                return Ok(InsertResponse {
                    subscriber_id,
                    new: false,
                });
            } else {
                return Err(SubscribeError::UnexpectedError(
                    anyhow::Error::new(sqlx::Error::Database(db_err))
                        .context("Failed to insert subscriber into database"),
                ));
            }
        }
        Err(e) => {
            return Err(SubscribeError::UnexpectedError(
                anyhow::Error::new(e).context("Failed to insert subscriber into database"),
            ));
        }
    }
}

#[instrument(
    name = "Sending confirmation email",
    skip(email_client, new_subscriber)
)]
pub async fn send_confirmation_email(
    email_client: &EmailClient,
    new_subscriber: NewSubscriber,
    application_base_url: &str,
    subscription_token: &str,
) -> Result<(), reqwest::Error> {
    let confirmation_link = format!(
        "{application_base_url}/subscriptions/confirm?subscription_token={subscription_token}"
    );
    let subject = "Welcome!";
    let html_body = format!(
        "Welcome to our newsletter!<br />\
        Click <a href=\"{}\"> here</a> to confirm your subscription.",
        confirmation_link
    );
    let text_body = format!(
        "Welcome to our newsletter!\n
        Visit {} to confirm your subscription.",
        confirmation_link
    );

    email_client
        .send_email(new_subscriber.email, subject, &text_body, &html_body)
        .await?;
    Ok(())
}

#[derive(thiserror::Error)]
pub enum SubscribeError {
    #[error("{0}")]
    ValidationError(String),
    #[error("User already confirmed")]
    AlreadyConfirmedError,
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for SubscribeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for SubscribeError {
    fn status_code(&self) -> StatusCode {
        match &self {
            Self::ValidationError(_) => StatusCode::BAD_REQUEST,
            Self::AlreadyConfirmedError => StatusCode::CONFLICT,
            Self::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

pub struct StoreTokenError(sqlx::Error);

impl std::error::Error for StoreTokenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

impl std::fmt::Display for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "A database error was encountered whilst trying to store a \
            subscription token."
        )
    }
}

fn error_chain_fmt(
    e: &impl std::error::Error,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    writeln!(f, "{}\n", e)?;
    let mut current = e.source();
    while let Some(s) = current {
        writeln!(f, "Caused by:\n\t {}", s)?;
        current = s.source();
    }
    Ok(())
}

impl std::fmt::Debug for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(&self, f)
    }
}

#[instrument(
    name = "Store subscription token in the database",
    skip(subscription_token, transaction)
)]
pub async fn store_token(
    subscriber_id: Uuid,
    subscription_token: &str,
    transaction: &mut Transaction<'_, Postgres>,
) -> Result<(), StoreTokenError> {
    sqlx::query!(
        r#"INSERT INTO subscription_tokens (subscriber_id, subscription_token) VALUES ($1, $2);"#,
        subscriber_id,
        subscription_token
    )
    .execute(&mut **transaction)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        StoreTokenError(e)
    })?;
    Ok(())
}

#[instrument(
    name = "Get the token from the subscriber id",
    skip(subscriber_id, pool)
)]
pub async fn get_token_from_subscriber_id(
    subscriber_id: &Uuid,
    pool: &PgPool,
) -> Result<String, sqlx::Error> {
    let result = sqlx::query!(
        r#"SELECT subscription_token FROM subscription_tokens WHERE subscriber_id = $1;"#,
        subscriber_id,
    )
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query {e:?}");
        e
    })?;
    Ok(result.subscription_token)
}

#[instrument(
    name = "Check duplicate user is unconfirmed",
    skip(subscriber_email, pool)
)]
pub async fn check_duplicate_user_is_unconfirmed(
    subscriber_email: &SubscriberEmail,
    pool: &PgPool,
) -> Result<Uuid, SubscribeError> {
    let subscriber = sqlx::query!(
        r#"SELECT id, status FROM subscriptions WHERE email = $1"#,
        subscriber_email.as_ref()
    )
    .fetch_one(pool)
    .await
    .context("Failed to get subscriber from email")?;

    match SubscriberStatus::from_string(subscriber.status) {
        SubscriberStatus::Confirmed => return Err(SubscribeError::AlreadyConfirmedError),
        SubscriberStatus::Pending => return Ok(subscriber.id),
    }
}

fn generate_subscription_token() -> String {
    let mut rng = rand::rng();
    (0..25).map(|_| rng.sample(Alphanumeric) as char).collect()
}
