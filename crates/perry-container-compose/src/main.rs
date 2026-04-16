use perry_container_compose::cli::{self, Cli};
use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();
    cli::run(cli).await?;
    Ok(())
}
