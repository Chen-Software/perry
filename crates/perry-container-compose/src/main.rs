use clap::Parser;
use perry_container_compose::cli::Cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _cli = Cli::parse();
    // Implementation
    Ok(())
}
