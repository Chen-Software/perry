//! `perry-compose` CLI using clap.

use crate::backend;
use crate::compose::ComposeEngine;
use crate::error::Result;
use crate::project::ComposeProject;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "perry-compose")]
#[command(version, about = "Docker Compose-like experience for OCI runtimes", long_about = None)]
pub struct Cli {
    #[arg(short, long, value_name = "FILE", help = "Compose file(s) [repeatable]")]
    pub file: Vec<String>,

    #[arg(short, long, value_name = "NAME", help = "Project name")]
    pub project_name: Option<String>,

    #[arg(long, value_name = "FILE", help = "Environment file(s) [repeatable]")]
    pub env_file: Vec<String>,

    #[arg(short = 'C', long, value_name = "DIR", help = "Change directory")]
    pub project_directory: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Clone)]
pub enum Commands {
    #[command(about = "Start services")]
    Up {
        #[arg(short, long, help = "Run in background")]
        detach: bool,
        #[arg(long, help = "Rebuild images before starting")]
        build: bool,
        #[arg(long, help = "Remove containers for undefined services")]
        remove_orphans: bool,
        #[arg(help = "Services to start (empty = all)")]
        services: Vec<String>,
    },
    #[command(about = "Stop and remove services")]
    Down {
        #[arg(short, long, help = "Remove named volumes")]
        volumes: bool,
        #[arg(long, help = "Remove containers for undefined services")]
        remove_orphans: bool,
    },
    #[command(about = "List service status")]
    Ps {
        #[arg(short, long, help = "Show all containers (including stopped)")]
        all: bool,
        #[arg(help = "Filter by service name")]
        services: Vec<String>,
    },
    #[command(about = "View output from containers")]
    Logs {
        #[arg(short, long, help = "Stream logs")]
        follow: bool,
        #[arg(long, value_name = "N", help = "Last N lines")]
        tail: Option<u32>,
        #[arg(short, long, help = "Show timestamps")]
        timestamps: bool,
        #[arg(help = "Services to show logs for (empty = all)")]
        services: Vec<String>,
    },
    #[command(about = "Execute a command in a running service")]
    Exec {
        #[arg(help = "Service name")]
        service: String,
        #[arg(help = "Command to run", trailing_var_arg = true)]
        cmd: Vec<String>,
        #[arg(short, long, help = "Environment variables")]
        env: Vec<String>,
        #[arg(short, long, help = "Working directory")]
        workdir: Option<String>,
        #[arg(short, long, help = "User context")]
        user: Option<String>,
    },
    #[command(about = "Validate and print resolved configuration")]
    Config {
        #[arg(long, default_value = "yaml", help = "Output format: yaml or json")]
        format: String,
        #[arg(long, help = "Resolve image digests")]
        resolve_image_digests: bool,
    },
    #[command(about = "Start existing stopped services")]
    Start {
        #[arg(help = "Services to start")]
        services: Vec<String>,
    },
    #[command(about = "Stop running services")]
    Stop {
        #[arg(help = "Services to stop")]
        services: Vec<String>,
    },
    #[command(about = "Restart services")]
    Restart {
        #[arg(help = "Services to restart")]
        services: Vec<String>,
    },
}

pub async fn run() -> Result<()> {
    let cli = Cli::parse();
    let project_dir = cli.project_directory.unwrap_or_else(|| std::env::current_dir().unwrap());

    let project = ComposeProject::load(
        cli.project_name,
        cli.file,
        cli.env_file,
        &project_dir,
    )?;

    let backend = backend::detect_backend().await.map_err(|probed| {
        crate::error::ComposeError::NoBackendFound { probed }
    })?;

    let engine = ComposeEngine::new(project.spec, project.project_name, Arc::new(backend));

    match cli.command {
        Commands::Up { detach, build, remove_orphans, services } => {
            engine.up(&services, detach, build, remove_orphans).await?;
        }
        Commands::Down { volumes, remove_orphans } => {
            engine.down(volumes, remove_orphans).await?;
        }
        Commands::Ps { all: _, services: _ } => {
            let infos = engine.ps().await?;
            for info in infos {
                println!("{:<20} {:<20} {:<20} {:<20}", info.name, info.image, info.status, info.id);
            }
        }
        Commands::Logs { follow: _, tail, timestamps: _, services } => {
            for svc in services {
                let logs = engine.logs(Some(&svc), tail).await?;
                print!("{}", logs.stdout);
                eprint!("{}", logs.stderr);
            }
        }
        Commands::Exec { service, cmd, env: _, workdir: _, user: _ } => {
            let logs = engine.exec(&service, &cmd).await?;
            print!("{}", logs.stdout);
            eprint!("{}", logs.stderr);
        }
        Commands::Config { format: _, resolve_image_digests: _ } => {
            println!("{}", engine.spec.to_yaml()?);
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
