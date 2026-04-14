use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use chrono::{DateTime, Local, TimeDelta};
use rand::random_range;
use sqlx::PgPool;
use tokio::time::sleep;
use tracing::field::Empty;
use tracing::instrument;
use url::Url;
use uuid::Uuid;

use crate::configuration::Settings;
use crate::domain::SubscriberEmail;
use crate::email_client::EmailClient;
use crate::idempotency::PgTransaction;
use crate::startup::get_connection_pool;

#[derive(Debug)]
pub enum QueueState {
    Waiting(f64),
    Empty,
    Ready(Task),
}

#[derive(Clone, Debug)]
pub struct Retries(u8);

impl Retries {
    pub fn from_u8(retry: u8) -> Self {
        Self(retry)
    }
    pub fn from_db(retry: i16) -> Self {
        Self(retry.try_into().expect("failed to fit retry into u8"))
    }
    pub fn to_db(&self) -> i16 {
        self.0 as i16
    }
}

#[derive(Debug)]
pub struct Task {
    newsletter_id: Uuid,
    subscriber_id: Uuid,
    subscriber_email: SubscriberEmail,
    title: String,
    content_text: String,
    content_html: String,
    retries: Retries,
}

pub async fn run_worker_until_stopped(configuration: Arc<Settings>) -> anyhow::Result<()> {
    let pool = get_connection_pool(&configuration.database);
    let sender_email = configuration
        .email_client
        .sender()
        .expect("Invalid sender email address");
    let base_url =
        Url::parse(&configuration.email_client.base_url).expect("Invalid url in configuration");
    let email_client = EmailClient::new(
        base_url,
        sender_email,
        configuration.email_client.authorization_token.clone(),
        configuration.email_client.timeout(),
    );
    worker_loop(&pool, &email_client, &configuration.application.max_retries).await
}

#[instrument(name="Dequeueing task", skip(tx), fields(newsletter_id=Empty, subscriber_id=Empty))]
pub async fn dequeue_task(tx: &mut PgTransaction) -> anyhow::Result<QueueState> {
    match sqlx::query!(
        r#"
    SELECT 
       t.subscriber_id,
       t.newsletter_id,
       s.email,
       n.title,
       n.content_text,
       n.content_html,
       t.retries
    FROM email_processing_tasks t
    JOIN subscriptions s ON s.id = t.subscriber_id
    JOIN newsletters n ON n.id = t.newsletter_id
    WHERE t.execute_after < now()
    FOR UPDATE OF t 
    SKIP LOCKED LIMIT 1;
    "#
    )
    .fetch_optional(&mut **tx)
    .await?
    {
        Some(record) => {
            tracing::Span::current().record(
                "newsletter_id",
                tracing::field::display(record.newsletter_id),
            );
            tracing::Span::current().record(
                "subscriber_id",
                tracing::field::display(record.subscriber_id),
            );

            let subscriber_email = SubscriberEmail::parse(record.email)
                .map_err(|_| anyhow!("Failed to parsed error"))?;

            Ok(QueueState::Ready(Task {
                newsletter_id: record.newsletter_id,
                subscriber_id: record.subscriber_id,
                subscriber_email,
                title: record.title,
                content_text: record.content_text,
                content_html: record.content_html,
                retries: Retries::from_db(record.retries),
            }))
        }
        None => match get_wait_time_to_next_task(tx).await? {
            None => Ok(QueueState::Empty),
            Some(wait_time) => Ok(QueueState::Waiting(wait_time)),
        },
    }
}

pub async fn get_wait_time_to_next_task(
    tx: &mut PgTransaction,
) -> Result<Option<f64>, anyhow::Error> {
    let time_delta = match sqlx::query!(
        r#"
    SELECT execute_after
    FROM email_processing_tasks 
    ORDER BY execute_after ASC
    LIMIT 1;
    "#
    )
    .fetch_optional(&mut **tx)
    .await?
    {
        None => return Ok(None),
        Some(r) => r.execute_after,
    };

    let duration = time_delta
        .signed_duration_since(chrono::Utc::now())
        .to_std()?;

    Ok(Some(duration.as_secs_f64()))
}

pub async fn delete_task(task: &Task, tx: &mut PgTransaction) -> anyhow::Result<()> {
    sqlx::query!(
        r#"
    DELETE
    FROM email_processing_tasks
    WHERE newsletter_id = $1 and subscriber_id = $2;
    "#,
        task.newsletter_id,
        task.subscriber_id
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn exponential_backoff_with_jitter(attempt: &u8) -> DateTime<Local> {
    let max_seconds = 2 << attempt;

    let time_differential_seconds: f64 = random_range(0.0..=(max_seconds as f64));
    let time_delta = TimeDelta::from_std(Duration::from_secs_f64(time_differential_seconds))
        .expect("time delta out of range");
    let now = Local::now();

    now.checked_add_signed(time_delta)
        .expect("time out of range")
}

pub async fn retry_task(
    task: &Task,
    max_retries: &u8,
    tx: &mut PgTransaction,
) -> anyhow::Result<()> {
    if task.retries.0 > 1 {
        let new_retries = task.retries.to_db() - 1;
        let attempt = max_retries - task.retries.0;
        let execute_after = exponential_backoff_with_jitter(&attempt);
        sqlx::query!(
            r#"
    UPDATE email_processing_tasks 
    SET 
      retries=$3,
      execute_after=$4
    WHERE newsletter_id = $1 and subscriber_id = $2;
    "#,
            task.newsletter_id,
            task.subscriber_id,
            new_retries,
            execute_after,
        )
        .execute(&mut **tx)
        .await?;
    } else {
        delete_task(task, tx).await?;
    }
    Ok(())
}

#[instrument(name="processing email", skip(pool,email_client, n_retries), fields(subscriber_email=Empty, title=Empty))]
pub async fn process_email(
    pool: &PgPool,
    email_client: &EmailClient,
    n_retries: &u8,
) -> anyhow::Result<QueueState> {
    let mut tx = pool.begin().await?;
    let queue_element = dequeue_task(&mut tx).await?;
    match queue_element {
        QueueState::Empty => Ok(QueueState::Empty),
        QueueState::Waiting(wait_time) => Ok(QueueState::Waiting(wait_time)),
        QueueState::Ready(task) => {
            tracing::Span::current().record(
                "subscriber_email",
                tracing::field::display(&task.subscriber_email),
            );
            tracing::Span::current().record("title", tracing::field::display(&task.title));
            match email_client
                .send_email(
                    &task.subscriber_email,
                    &task.title,
                    &task.content_text,
                    &task.content_html,
                )
                .await
            {
                Ok(()) => {
                    delete_task(&task, &mut tx).await?;
                    tx.commit().await?;
                    Ok(QueueState::Ready(task))
                }
                Err(e) => {
                    retry_task(&task, n_retries, &mut tx).await?;
                    tx.commit().await?;
                    Err(anyhow!(e.to_string()))
                }
            }
        }
    }
}

pub async fn worker_loop(
    pool: &PgPool,
    email_client: &EmailClient,
    n_retries: &u8,
) -> anyhow::Result<()> {
    loop {
        match process_email(pool, email_client, n_retries).await {
            Ok(QueueState::Empty) => sleep(Duration::from_secs(10)).await,
            Ok(QueueState::Ready(_)) => (),
            Ok(QueueState::Waiting(wait_time)) => sleep(Duration::from_secs_f64(wait_time)).await,
            Err(_) => (),
        }
    }
}
