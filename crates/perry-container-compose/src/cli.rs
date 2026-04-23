use clap::{Parser, Subcommand};
use std::path::PathBuf;
use crate::error::Result;
use crate::project::ComposeProject;
use crate::config::ProjectConfig;
use crate::compose::ComposeEngine;
use crate::backend::detect_backend;
use crate::service;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "perry-compose")]
#[command(version = "0.5.56")]
#[command(about = "Native container-compose for Perry", long_about = None)]
pub struct Cli {
    #[arg(short, long, value_name = "FILE", action = clap::ArgAction::Append)]
    pub file: Vec<PathBuf>,

    #[arg(short, long, value_name = "NAME")]
    pub project_name: Option<String>,

    #[arg(long, value_name = "PATH", action = clap::ArgAction::Append)]
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
        #[arg(trailing_var_arg = true)]
        services: Vec<String>,
    },
    Exec {
        service: String,
        command: Vec<String>,
        #[arg(short, long, action = clap::ArgAction::Append)]
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
        files: cli.file,
        project_name: cli.project_name,
        env_files: cli.env_file,
    };

    let project = ComposeProject::load(config)?;
    let backend = detect_backend().await?;
    let engine = ComposeEngine::new(project.spec, project.project_name, Arc::from(backend));

    match cli.command {
        Commands::Up { detach, build, remove_orphans, .. } => {
            engine.up(detach, build, remove_orphans).await?;
        }
        Commands::Down { volumes, remove_orphans, .. } => {
            engine.down(volumes, remove_orphans).await?;
        }
        Commands::Ps { .. } => {
            let infos = engine.ps().await?;
            println!("{:<20} {:<20} {:<20} {:<20}", "ID", "NAME", "IMAGE", "STATUS");
            for info in infos {
                println!("{:<20} {:<20} {:<20} {:<20}", info.id, info.name, info.image, info.status);
            }
        }
        Commands::Logs { services, tail, .. } => {
            let container_name = if let Some(svc) = services.first() {
                service::generate_name(&engine.project_name, svc, engine.spec.services.get(svc).unwrap())?
            } else {
                String::new()
            };
            let logs = engine.backend.logs(&container_name, tail).await?;
            print!("{}", logs.stdout);
            eprint!("{}", logs.stderr);
        }
        Commands::Exec { service, command, .. } => {
            let container_name = service::generate_name(&engine.project_name, &service, engine.spec.services.get(&service).unwrap())?;
            let logs = engine.backend.exec(&container_name, &command, None, None).await?;
            print!("{}", logs.stdout);
            eprint!("{}", logs.stderr);
        }
        Commands::Config { format, .. } => {
            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&engine.spec).unwrap());
            } else {
                println!("{}", serde_yaml::to_string(&engine.spec).unwrap());
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
