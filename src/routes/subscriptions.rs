use actix_web::{HttpResponse, web};
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
) -> HttpResponse {
    let new_subscriber: NewSubscriber = match form.0.try_into() {
        Ok(new_subcriber) => new_subcriber,
        Err(_) => return HttpResponse::BadRequest().finish(),
    };
    let mut transaction = match connection.begin().await {
        Ok(transaction) => transaction,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    let insert_response =
        match insert_subscriber(&mut transaction, &connection, &new_subscriber).await {
            Ok(subscriber_id) => subscriber_id,
            Err(SubscriberError::AlreadyConfirmed) => {
                return HttpResponse::Conflict().finish();
            }
            Err(_) => return HttpResponse::InternalServerError().finish(),
        };

    let subscription_token = if insert_response.new {
        let subscription_token = generate_subscription_token();
        if store_token(
            insert_response.subscriber_id,
            &subscription_token,
            &mut transaction,
        )
        .await
        .is_err()
        {
            return HttpResponse::InternalServerError().finish();
        }
        subscription_token
    } else {
        match get_token_from_subscriber_id(&insert_response.subscriber_id, &connection).await {
            Ok(token) => token,
            Err(_) => {
                return HttpResponse::InternalServerError().finish();
            }
        }
    };

    if transaction.commit().await.is_err() {
        return HttpResponse::InternalServerError().finish();
    };

    match send_confirmation_email(
        &email_client,
        new_subscriber,
        &application_base_url.0,
        &subscription_token,
    )
    .await
    {
        Err(e) => {
            tracing::error!("Failed to send confirmation email {e:?}");
            return HttpResponse::InternalServerError().finish();
        }
        Ok(_) => return HttpResponse::Ok().finish(),
    }
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
) -> Result<InsertResponse, SubscriberError> {
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
                return Err(SubscriberError::Sqlx(sqlx::Error::Database(db_err)));
            }
        }
        Err(e) => {
            return Err(SubscriberError::Sqlx(e));
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

#[instrument(
    name = "Store subscription token in the database",
    skip(subscription_token, transaction)
)]
pub async fn store_token(
    subscriber_id: Uuid,
    subscription_token: &str,
    transaction: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"INSERT INTO subscription_tokens (subscriber_id, subscription_token) VALUES ($1, $2);"#,
        subscriber_id,
        subscription_token
    )
    .execute(&mut **transaction)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
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

pub enum SubscriberError {
    AlreadyConfirmed,
    Sqlx(sqlx::Error),
}
#[instrument(
    name = "Check duplicate user is unconfirmed",
    skip(subscriber_email, pool)
)]
pub async fn check_duplicate_user_is_unconfirmed(
    subscriber_email: &SubscriberEmail,
    pool: &PgPool,
) -> Result<Uuid, SubscriberError> {
    let subscriber = sqlx::query!(
        r#"SELECT id, status FROM subscriptions WHERE email = $1"#,
        subscriber_email.as_ref()
    )
    .fetch_one(pool)
    .await
    .map_err(SubscriberError::Sqlx)?;

    match SubscriberStatus::from_string(subscriber.status) {
        SubscriberStatus::Confirmed => return Err(SubscriberError::AlreadyConfirmed),
        SubscriberStatus::Pending => return Ok(subscriber.id),
    }
}

fn generate_subscription_token() -> String {
    let mut rng = rand::rng();
    (0..25).map(|_| rng.sample(Alphanumeric) as char).collect()
}
