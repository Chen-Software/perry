use clap::{Parser, Subcommand};
use std::path::PathBuf;
use crate::config::ProjectConfig;
use crate::project::ComposeProject;
use crate::compose::ComposeEngine;
use crate::backend::detect_backend;
use crate::error::Result;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "perry-compose")]
#[command(about = "Docker Compose-like experience for Apple Container", long_about = None)]
pub struct Cli {
    #[arg(short, long, value_name = "FILE")]
    pub file: Vec<PathBuf>,

    #[arg(short, long, value_name = "NAME")]
    pub project_name: Option<String>,

    #[arg(long, value_name = "PATH")]
    pub env_file: Vec<PathBuf>,

    #[arg(long, value_name = "PATH")]
    pub project_directory: Option<PathBuf>,

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

        #[arg(long)]
        resolve_image_digests: bool,
    },
    Start { services: Vec<String> },
    Stop { services: Vec<String> },
    Restart { services: Vec<String> },
}

pub async fn run_cli() -> Result<()> {
    let cli = Cli::parse();

    let config = ProjectConfig {
        project_name: cli.project_name,
        compose_files: cli.file,
        env_files: cli.env_file,
        project_dir: cli.project_directory,
    };

    let project = ComposeProject::load(config)?;
    let backend = detect_backend().await?;
    let engine = Arc::new(ComposeEngine::new(project.spec, project.project_name, Arc::from(backend)));

    match cli.command {
        Commands::Up { build, .. } => {
            engine.up(build).await?;
        }
        Commands::Down { volumes, .. } => {
            engine.down(volumes).await?;
        }
        Commands::Ps { .. } => {
            let list = engine.ps().await?;
            for c in list {
                println!("{:<20} {:<20} {:<20} {}", c.name, c.image, c.status, c.id);
            }
        }
        Commands::Logs { services, tail, .. } => {
            let logs = engine.logs(services.first().map(|s| s.as_str()), tail).await?;
            print!("{}", logs.stdout);
            eprint!("{}", logs.stderr);
        }
        Commands::Exec { service, command, .. } => {
            let logs = engine.exec(&service, &command).await?;
            print!("{}", logs.stdout);
            eprint!("{}", logs.stderr);
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
        Commands::Config { format, .. } => {
            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&engine.spec).unwrap());
            } else {
                println!("{}", serde_yaml::to_string(&engine.spec).unwrap());
            }
        }
    }

    Ok(())
}
