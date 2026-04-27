use perry_container_compose::error::Result;
use perry_container_compose::cli;

use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cli = cli::Cli::parse();
    cli::run(cli).await
}
