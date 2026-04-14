use anyhow::Result;
use std::{
    fmt::{Debug, Display},
    sync::Arc,
};
use tokio::task::JoinError;
use zero2prod::{
    configuration::get_configuration,
    idempotency, issue_delivery_worker, startup,
    telementry::{get_subscriber, init_subscriber},
};

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);
    let configuration = Arc::new(get_configuration().expect("Failed to get config"));
    let application = startup::Application::build(&configuration).await?;
    let application_task = tokio::spawn(application.run_until_stopped());
    let email_worker = tokio::spawn(issue_delivery_worker::run_worker_until_stopped(
        configuration.clone(),
    ));
    let idempotency_cleaner = tokio::spawn(idempotency::run_until_stopped(configuration.clone()));
    tokio::select!(
    o = application_task => report_exit("API",o),
    o = email_worker => report_exit("email worker", o),
    o = idempotency_cleaner => report_exit("idempotency cleaner", o)
    );
    Ok(())
}

fn report_exit(task_name: &str, outcome: Result<Result<(), impl Debug + Display>, JoinError>) {
    match outcome {
        Ok(Ok(())) => {
            tracing::info!("{} has exited", task_name)
        }
        Ok(Err(e)) => {
            tracing::error!(
            error.cause_chain = ?e,
            error.message = %e,
            "{} failed",
            task_name
            )
        }
        Err(e) => {
            tracing::error!(
            error.cause_chain = ?e,
            error.message = %e,
            "{}' task failed to complete",
            task_name
            )
        }
    }
}
