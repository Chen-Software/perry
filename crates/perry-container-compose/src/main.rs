use perry_container_compose::error::Result;
use perry_container_compose::cli;

#[tokio::main]
async fn main() -> Result<()> {
    use clap::Parser;
    tracing_subscriber::fmt::init();
    let cli = cli::Cli::parse();
    cli::run(cli).await
}
