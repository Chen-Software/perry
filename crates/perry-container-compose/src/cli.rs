use clap::{Parser, Subcommand};
use std::path::PathBuf;
use crate::error::Result;
use crate::project::ComposeProject;
use crate::config::ProjectConfig;
use crate::compose::ComposeEngine;
use crate::backend::detect_backend;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "perry-compose")]
#[command(version)]
pub struct Cli {
    #[arg(short, long, global = true)]
    pub file: Vec<String>,

    #[arg(short, long, global = true)]
    pub project_name: Option<String>,

    #[arg(long, global = true)]
    pub env_file: Vec<String>,

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
        #[arg(long)]
        resolve_image_digests: bool,
    },
    Start { services: Vec<String> },
    Stop { services: Vec<String> },
    Restart { services: Vec<String> },
}

pub async fn run() -> Result<()> {
    let cli = Cli::parse();
    let project_config = ProjectConfig {
        files: cli.file.into_iter().map(PathBuf::from).collect(),
        project_name: cli.project_name,
        env_files: cli.env_file.into_iter().map(PathBuf::from).collect(),
    };

    let project = ComposeProject::load(project_config)?;
    let backend = detect_backend().await.map_err(|_| crate::error::ComposeError::BackendNotAvailable {
         name: "auto".into(),
         reason: "No container backend found".into(),
    })?;
    let engine = ComposeEngine::new(project.spec, backend.into_arc());

    match cli.command {
        Commands::Up { build, .. } => {
            engine.up(build).await?;
        }
        Commands::Down { volumes, .. } => {
            engine.down(volumes).await?;
        }
        Commands::Ps { .. } => {
            let info = engine.ps().await?;
            println!("{:<20} {:<20} {:<20} {:<20}", "NAME", "IMAGE", "STATUS", "PORTS");
            for c in info {
                println!("{:<20} {:<20} {:<20} {:<20?}", c.name, c.image, c.status, c.ports);
            }
        }
        Commands::Logs { services, tail, .. } => {
            let logs = engine.logs(services.first().map(|s: &String| s.as_str()), tail).await?;
            print!("{}", logs.stdout);
            eprint!("{}", logs.stderr);
        }
        Commands::Exec { service, command, .. } => {
            let logs = engine.exec(&service, &command).await?;
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
