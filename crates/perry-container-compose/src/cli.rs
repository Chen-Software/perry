//! CLI implementation for perry-compose.

use crate::backend::detect_backend;
use crate::compose::ComposeEngine;
use crate::error::Result;
use crate::project::ComposeProject;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "perry-compose")]
#[command(about = "Docker Compose-like experience for Apple Container / Podman", long_about = None)]
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

        /// Services to remove
        services: Vec<String>,
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
        #[arg(required = true)]
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
    Start { services: Vec<String> },

    /// Stop running services
    Stop { services: Vec<String> },

    /// Restart services
    Restart { services: Vec<String> },
}

pub async fn run_cli() -> Result<()> {
    let cli = Cli::parse();

    // 1. Detect backend
    let backend = Arc::new(detect_backend().await?);

    // 2. Load project
    let project = ComposeProject::load_from_files(
        &cli.file,
        cli.project_name.as_deref(),
        &cli.env_file,
    )?;

    // 3. Initialize engine
    let engine = ComposeEngine::new(project.spec, project.project_name, backend);

    // 4. Dispatch command
    match cli.command {
        Commands::Up { detach, build, remove_orphans, services } => {
            engine.up(&services, detach, build, remove_orphans).await?;
        }
        Commands::Down { volumes, remove_orphans, services: _ } => {
            engine.down(volumes, remove_orphans).await?;
        }
        Commands::Ps { all: _, services: _ } => {
            let infos = engine.ps().await?;
            for info in infos {
                println!("{:<20} {:<20} {:<20} {:<20}", info.name, info.image, info.status, info.id);
            }
        }
        Commands::Logs { follow: _, tail, timestamps: _, services } => {
            let svc = services.first().map(|s| s.as_str());
            let logs = engine.logs(svc, tail).await?;
            print!("{}", logs.stdout);
            eprint!("{}", logs.stderr);
        }
        Commands::Exec { service, cmd, env: _, workdir: _, user: _ } => {
            let logs = engine.exec(&service, &cmd).await?;
            print!("{}", logs.stdout);
            eprint!("{}", logs.stderr);
        }
        Commands::Config { format, resolve_image_digests: _ } => {
            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&engine).unwrap_or_default());
            } else {
                // TODO: Spec to YAML
            }
        }
        Commands::Start { services } => engine.start(&services).await?,
        Commands::Stop { services } => engine.stop(&services).await?,
        Commands::Restart { services } => engine.restart(&services).await?,
    }

    Ok(())
}
