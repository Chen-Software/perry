use clap::{Parser, Subcommand};
use crate::project::ComposeProject;
use crate::compose::ComposeEngine;
use crate::backend::detect_backend;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "perry-compose", version, about = "OCI container orchestration")]
pub struct Cli {
    #[arg(short, long)]
    pub file: Vec<PathBuf>,
    #[arg(short, long)]
    pub project_name: Option<String>,
    #[arg(long)]
    pub env_file: Vec<PathBuf>,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Up {
        #[arg(short, long)]
        detach: bool,
        #[arg(long)]
        build: bool,
        #[arg(long)]
        remove_orphans: bool,
        services: Vec<String>,
    },
    Down {
        #[arg(short, long)]
        volumes: bool,
        #[arg(long)]
        remove_orphans: bool,
        services: Vec<String>,
    },
    Ps {
        #[arg(short, long)]
        all: bool,
        services: Vec<String>,
    },
    Logs {
        #[arg(short, long)]
        follow: bool,
        #[arg(long)]
        tail: Option<u32>,
        #[arg(short, long)]
        timestamps: bool,
        services: Vec<String>,
    },
    Exec {
        service: String,
        #[arg(trailing_var_arg = true)]
        command: Vec<String>,
        #[arg(short, long)]
        env: Vec<String>,
        #[arg(short, long)]
        workdir: Option<String>,
        #[arg(short, long)]
        user: Option<String>,
    },
    Config {
        #[arg(long, default_value = "yaml")]
        format: String,
    },
    Start { services: Vec<String> },
    Stop { services: Vec<String> },
    Restart { services: Vec<String> },
}

pub async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let project = ComposeProject::new(cli.project_name, &cli.file, &cli.env_file, None)?;
    let backend = detect_backend().await.map_err(|e| anyhow::anyhow!("No backend: {:?}", e))?;
    let engine = ComposeEngine::new(project.spec, project.name, Arc::from(backend));

    match cli.command {
        Commands::Up { detach, build, remove_orphans, services } => {
            engine.up(&services, detach, build, remove_orphans).await?;
        }
        Commands::Down { volumes, remove_orphans, services } => {
            engine.down(&services, remove_orphans, volumes).await?;
        }
        Commands::Ps { services: _, .. } => {
            let infos = engine.ps().await?;
            for info in infos { println!("{}: {}", info.name, info.status); }
        }
        _ => println!("Command not yet fully implemented in CLI wrapper"),
    }
    Ok(())
}
