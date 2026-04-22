use clap::{Parser, Subcommand};
use std::path::PathBuf;
use crate::config::ProjectConfig;
use crate::project::ComposeProject;
use crate::compose::ComposeEngine;
use crate::backend::{detect_backend, ComposeError};
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "perry-compose")]
#[command(about = "Docker Compose-like experience for Perry", long_about = None)]
pub struct Cli {
    #[arg(short, long, global = true)]
    pub file: Vec<PathBuf>,

    #[arg(short, long, global = true)]
    pub project_name: Option<String>,

    #[arg(long, global = true)]
    pub env_file: Vec<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Clone)]
pub enum Commands {
    /// Start services
    Up {
        #[arg(short, long)]
        detach: bool,

        #[arg(long)]
        build: bool,

        #[arg(long)]
        remove_orphans: bool,

        services: Vec<String>,
    },
    /// Stop and remove services
    Down {
        #[arg(short, long)]
        volumes: bool,

        #[arg(long)]
        remove_orphans: bool,

        services: Vec<String>,
    },
    /// List service status
    Ps {
        #[arg(short, long)]
        all: bool,

        services: Vec<String>,
    },
    /// View output from containers
    Logs {
        #[arg(short, long)]
        follow: bool,

        #[arg(long)]
        tail: Option<u32>,

        #[arg(short, long)]
        timestamps: bool,

        services: Vec<String>,
    },
    /// Execute a command in a running service
    Exec {
        service: String,
        cmd: Vec<String>,

        #[arg(short, long)]
        env: Vec<String>,

        #[arg(short, long)]
        workdir: Option<String>,

        #[arg(short, long)]
        user: Option<String>,
    },
    /// Validate and print resolved configuration
    Config {
        #[arg(long, default_value = "yaml")]
        format: String,

        #[arg(long)]
        resolve_image_digests: bool,
    },
    /// Start existing stopped services
    Start {
        services: Vec<String>,
    },
    /// Stop running services
    Stop {
        services: Vec<String>,
    },
    /// Restart services
    Restart {
        services: Vec<String>,
    },
}

pub async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = ProjectConfig::resolve(cli.file, cli.project_name, cli.env_file);
    let project = ComposeProject::load(&config)?;
    let backend = detect_backend().await.map_err(|e| ComposeError::NoBackendFound { probed: e })?;
    let engine = ComposeEngine::new(project.spec, backend);

    match cli.command {
        Commands::Up { detach, .. } => {
            engine.up().await?;
        }
        Commands::Down { volumes, .. } => {
            engine.down(volumes).await?;
        }
        Commands::Ps { .. } => {
            let list = engine.ps().await?;
            println!("{:<20} {:<20} {:<20} {:<20}", "ID", "NAME", "IMAGE", "STATUS");
            for info in list {
                println!("{:<20} {:<20} {:<20} {:<20}", info.id, info.name, info.image, info.status);
            }
        }
        Commands::Logs { services, tail, .. } => {
            let logs = engine.logs(services.first().map(|s: &String| s.as_str()), tail).await?;
            print!("{}", logs.stdout);
            eprint!("{}", logs.stderr);
        }
        Commands::Exec { service, cmd, .. } => {
            let logs = engine.exec(&service, &cmd).await?;
            print!("{}", logs.stdout);
            eprint!("{}", logs.stderr);
        }
        Commands::Config { format, .. } => {
            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&engine.spec)?);
            } else {
                println!("{}", serde_yaml::to_string(&engine.spec)?);
            }
        }
        Commands::Start { services } => {
            engine.start(&services).await?;
        }
        Commands::Stop { services } => {
            engine.stop(&services).await?;
        }
        Commands::Restart { services } => {
            engine.restart(&services).await?;
        }
    }

    Ok(())
}
