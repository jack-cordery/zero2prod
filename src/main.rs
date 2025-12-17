use std::{io::Result, net::TcpListener};
use zero2prod::{configuration::get_configuration, startup};

#[tokio::main]
async fn main() -> Result<()> {
    let configuration = get_configuration().expect("Failed to read configuration");
    let application_address = format!("127.0.0.1:{}", configuration.application_port);
    let listener = TcpListener::bind(application_address)?;
    startup::run(listener)?.await
}
