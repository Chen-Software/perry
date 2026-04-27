use clap::Parser;
use perry_container_compose::cli::{Cli, Commands};
use perry_container_compose::project::ComposeProject;
use perry_container_compose::compose::ComposeEngine;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    let project = ComposeProject::load(cli.file, cli.project_name)?;
    let engine = ComposeEngine::new(project.spec).await?;

    match cli.command {
        Commands::Up { .. } => {
            engine.up().await?;
        }
        Commands::Down { .. } => {
            // engine.down().await?;
        }
        Commands::Ps => {
            // engine.ps().await?;
        }
        Commands::Logs { .. } => {
            // engine.logs().await?;
        }
    }

    Ok(())
}
