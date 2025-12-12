use std::{io::Result, net::TcpListener};
use zero2prod::run;

#[tokio::main]
async fn main() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    run(listener)?.await
}
