use perry_container_compose::cli::{Cli, Commands};
use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Up { .. } => println!("Up functionality not fully wired in CLI yet"),
        _ => println!("Command not implemented"),
    }

    Ok(())
}
