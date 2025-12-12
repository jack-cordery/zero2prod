use std::io::Result;
use zero2prod::run;

#[tokio::main]
async fn main() -> Result<()> {
    run()?.await
}
