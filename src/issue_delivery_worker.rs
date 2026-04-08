use std::time::Duration;

use anyhow::anyhow;
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
// we want to create a worker that will pick up email tasks and process them
//
pub enum QueueState {
    Empty,
    NonEmpty(Task),
}

pub struct Task {
    newsletter_id: Uuid,
    subscriber_id: Uuid,
    subscriber_email: SubscriberEmail,
    title: String,
    content_text: String,
    content_html: String,
}

pub async fn run_worker_until_stopped(configuration: Settings) -> anyhow::Result<()> {
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
    worker_loop(&pool, &email_client).await
}

#[instrument(name="Dequeueing task", skip(tx), fields(newsletter_id=Empty, subscriber_id=Empty))]
pub async fn dequeue_task(tx: &mut PgTransaction) -> anyhow::Result<QueueState> {
    // gets the email (title, text_content, html_content) and subscriber email for processing
    match sqlx::query!(
        r#"
    SELECT 
       t.subscriber_id,
       t.newsletter_id,
       s.email,
       n.title,
       n.content_text,
       n.content_html
    FROM email_processing_tasks t
    JOIN subscriptions s ON s.id = t.subscriber_id
    JOIN newsletters n ON n.id = t.newsletter_id
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

            Ok(QueueState::NonEmpty(Task {
                newsletter_id: record.newsletter_id,
                subscriber_id: record.subscriber_id,
                subscriber_email,
                title: record.title,
                content_text: record.content_text,
                content_html: record.content_html,
            }))
        }
        None => Ok(QueueState::Empty),
    }
}

pub async fn delete_task(
    newsletter_id: &Uuid,
    subscriber_id: &Uuid,
    tx: &mut PgTransaction,
) -> anyhow::Result<()> {
    // now all we need to do is delete those values from the table
    sqlx::query!(
        r#"
    DELETE
    FROM email_processing_tasks
    WHERE newsletter_id = $1 and subscriber_id = $2;
    "#,
        newsletter_id,
        subscriber_id
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[instrument(name="processing email", skip(pool,email_client), fields(subscriber_email=Empty, title=Empty))]
pub async fn process_email(
    pool: &PgPool,
    email_client: &EmailClient,
) -> anyhow::Result<QueueState> {
    let mut tx = pool.begin().await?;
    let queue_element = dequeue_task(&mut tx).await?;
    match queue_element {
        QueueState::Empty => Ok(QueueState::Empty),
        QueueState::NonEmpty(task) => {
            tracing::Span::current().record(
                "subscriber_email",
                tracing::field::display(&task.subscriber_email),
            );
            tracing::Span::current().record("title", tracing::field::display(&task.title));
            email_client
                .send_email(
                    &task.subscriber_email,
                    &task.title,
                    &task.content_text,
                    &task.content_html,
                )
                .await?;
            delete_task(&task.newsletter_id, &task.subscriber_id, &mut tx).await?;
            tx.commit().await?;
            Ok(QueueState::NonEmpty(task))
        }
    }
}

pub async fn worker_loop(pool: &PgPool, email_client: &EmailClient) -> anyhow::Result<()> {
    loop {
        match process_email(pool, email_client).await {
            Ok(QueueState::Empty) => sleep(Duration::from_secs(10)).await,
            Ok(QueueState::NonEmpty(_)) => (),
            Err(_) => sleep(Duration::from_secs(1)).await,
        }
    }
}
