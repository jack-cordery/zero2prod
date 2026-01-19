use std::io::Result;
use zero2prod::{
    configuration::get_configuration,
    startup,
    telementry::{get_subscriber, init_subscriber},
};

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);
    let configuration = get_configuration().expect("Failed to get config");
    let server = startup::Application::build(&configuration).await?;
    server.run_until_stopped().await?;
    Ok(())
}
