use clap::{Parser, Subcommand};
use std::path::PathBuf;
use crate::config::ProjectConfig;
use crate::project::ComposeProject;
use crate::compose::ComposeEngine;
use crate::backend::get_global_backend_instance;

#[derive(Parser)]
#[command(name = "perry-compose")]
#[command(version = "1.0")]
#[command(about = "Docker Compose-like experience for Apple Container", long_about = None)]
pub struct Cli {
    #[arg(short, long, value_name = "FILE")]
    pub file: Vec<PathBuf>,

    #[arg(short, long, value_name = "NAME")]
    pub project_name: Option<String>,

    #[arg(long, value_name = "FILE")]
    pub env_file: Vec<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Clone)]
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
        #[arg(required = true)]
        cmd: Vec<String>,
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
    Start {
        services: Vec<String>,
    },
    Stop {
        services: Vec<String>,
    },
    Restart {
        services: Vec<String>,
    },
}

pub async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = ProjectConfig::resolve(cli.file, cli.project_name, cli.env_file);
    let project = ComposeProject::load(&config)?;
    let backend = get_global_backend_instance().await?;
    let engine = ComposeEngine::new(project.spec, backend);

    match cli.command {
        Commands::Up { .. } => {
            engine.up().await?;
        }
        Commands::Down { volumes, .. } => {
            engine.down(volumes).await?;
        }
        Commands::Ps { .. } => {
            let list = engine.ps().await?;
            for info in list {
                println!("{:<20} {:<20} {:<20}", info.name, info.image, info.status);
            }
        }
        Commands::Logs { services, tail, .. } => {
            let service = services.first().map(|s| s.as_str());
            let logs = engine.logs(service, tail).await?;
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
