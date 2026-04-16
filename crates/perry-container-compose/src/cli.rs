use clap::{Parser, Subcommand};
use std::path::PathBuf;
use crate::error::Result;
use crate::config::ProjectConfig;
use crate::project::ComposeProject;
use crate::compose::ComposeEngine;
use crate::backend::detect_backend;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "perry-compose")]
#[command(version = "1.0")]
#[command(about = "Docker Compose-like experience for Apple Container / Podman")]
pub struct Cli {
    /// Compose file(s)
    #[arg(short, long, global = true)]
    pub file: Vec<PathBuf>,

    /// Project name
    #[arg(short, long, global = true)]
    pub project_name: Option<String>,

    /// Environment file(s)
    #[arg(long, global = true)]
    pub env_file: Vec<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start services
    Up {
        /// Run in background
        #[arg(short, long)]
        detach: bool,

        /// Rebuild images before starting
        #[arg(long)]
        build: bool,

        /// Remove containers for undefined services
        #[arg(long)]
        remove_orphans: bool,

        /// Services to start
        services: Vec<String>,
    },
    /// Stop and remove services
    Down {
        /// Remove named volumes
        #[arg(short, long)]
        volumes: bool,

        /// Remove containers for undefined services
        #[arg(long)]
        remove_orphans: bool,
    },
    /// List service status
    Ps {
        /// Show all containers (including stopped)
        #[arg(short, long)]
        all: bool,

        /// Filter by service name
        services: Vec<String>,
    },
    /// View output from containers
    Logs {
        /// Stream logs
        #[arg(short, long)]
        follow: bool,

        /// Last N lines
        #[arg(long)]
        tail: Option<u32>,

        /// Show timestamps
        #[arg(short, long)]
        timestamps: bool,

        /// Services to show logs for
        services: Vec<String>,
    },
    /// Execute a command in a running service
    Exec {
        /// Service name
        service: String,

        /// Command to run
        cmd: Vec<String>,

        /// Environment variables
        #[arg(short, long)]
        env: Vec<String>,

        /// Working directory
        #[arg(short, long)]
        workdir: Option<String>,

        /// User context
        #[arg(short, long)]
        user: Option<String>,
    },
    /// Validate and print resolved configuration
    Config {
        /// Output format: yaml or json
        #[arg(long, default_value = "yaml")]
        format: String,

        /// Resolve image digests
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

pub async fn run_cli() -> Result<()> {
    let cli = Cli::parse();
    let project_dir = std::env::current_dir()?;
    let config = ProjectConfig::new(cli.file, cli.project_name, cli.env_file, project_dir);
    let project = ComposeProject::load(&config)?;

    let backend = detect_backend().await.map_err(|_| crate::error::ComposeError::NotFound("no backend found".into()))?;
    let engine = Arc::new(ComposeEngine::new(project.spec, project.project_name, Arc::new(backend)));

    match cli.command {
        Commands::Up { detach, build, remove_orphans, services } => {
            engine.up(&services, detach, build, remove_orphans).await?;
        }
        Commands::Down { volumes, remove_orphans } => {
            engine.down(volumes, remove_orphans).await?;
        }
        Commands::Ps { services, .. } => {
            let infos = engine.ps().await?;
            for info in infos {
                if services.is_empty() || services.contains(&info.name) {
                    println!("{:<20} {:<20} {:<20} {:<20}", info.name, info.image, info.status, info.id);
                }
            }
        }
        Commands::Logs { services, tail, .. } => {
            let s_name = if services.is_empty() { None } else { Some(services[0].as_str()) };
            let logs = engine.logs(s_name, tail).await?;
            println!("{}", logs.stdout);
            eprintln!("{}", logs.stderr);
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
                println!("{}", engine.spec.to_yaml()?);
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
