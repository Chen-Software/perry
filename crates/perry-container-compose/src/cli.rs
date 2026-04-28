//! CLI entry point for `perry-compose` binary.

use crate::compose::ComposeEngine;
use crate::error::Result;
use crate::project::ComposeProject;
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(name = "perry-compose", version, about = "Docker Compose-like CLI", long_about = None)]
pub struct Cli {
    #[arg(short = 'f', long = "file", value_name = "FILE", global = true)]
    pub files: Vec<PathBuf>,
    #[arg(short = 'p', long = "project-name", global = true)]
    pub project_name: Option<String>,
    #[arg(long = "env-file", value_name = "FILE", global = true)]
    pub env_files: Vec<PathBuf>,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Up(UpArgs),
    Down(DownArgs),
    Start(ServiceArgs),
    Stop(ServiceArgs),
    Restart(ServiceArgs),
    Ps(PsArgs),
    Logs(LogsArgs),
    Exec(ExecArgs),
    Config(ConfigArgs),
}

#[derive(Args, Debug)]
pub struct UpArgs {
    #[arg(short = 'd', long = "detach")]
    pub detach: bool,
    #[arg(long = "build")]
    pub build: bool,
    #[arg(long = "remove-orphans")]
    pub remove_orphans: bool,
    pub services: Vec<String>,
}

#[derive(Args, Debug)]
pub struct DownArgs {
    #[arg(short = 'v', long = "volumes")]
    pub volumes: bool,
    #[arg(long = "remove-orphans")]
    pub remove_orphans: bool,
    pub services: Vec<String>,
}

#[derive(Args, Debug)]
pub struct ServiceArgs { pub services: Vec<String> }

#[derive(Args, Debug)]
pub struct PsArgs { pub services: Vec<String> }

#[derive(Args, Debug)]
pub struct LogsArgs {
    #[arg(long = "tail")]
    pub tail: Option<u32>,
    pub services: Vec<String>,
}

#[derive(Args, Debug)]
pub struct ExecArgs {
    pub service: String,
    pub cmd: Vec<String>,
    pub workdir: Option<String>,
}

#[derive(Args, Debug)]
pub struct ConfigArgs {}

pub async fn run(cli: Cli) -> Result<()> {
    let config = crate::config::ProjectConfig::new(cli.files.clone(), cli.project_name.clone(), cli.env_files.clone());
    let project = ComposeProject::load(&config)?;
    let backend = crate::backend::detect_backend().await?;
    let engine = Arc::new(ComposeEngine::new(project.spec.clone(), project.project_name.clone(), backend));

    match cli.command {
        Commands::Up(args) => {
            Arc::clone(&engine).up(&args.services, args.detach, args.build, args.remove_orphans).await?;
        }
        Commands::Down(args) => {
            engine.down(&args.services, args.remove_orphans, args.volumes).await?;
        }
        Commands::Start(args) => { engine.start(&args.services).await?; }
        Commands::Stop(args) => { engine.stop(&args.services).await?; }
        Commands::Restart(args) => { engine.restart(&args.services).await?; }
        Commands::Ps(_) => {
            let infos = engine.ps().await?;
            for info in infos { println!("{:<24} {:<12} {:<36}", info.name, info.status, info.id); }
        }
        Commands::Logs(args) => {
            let svc = if args.services.is_empty() { None } else { Some(args.services[0].as_str()) };
            let logs = engine.logs(svc, args.tail).await?;
            print!("{}", logs.stdout);
            eprint!("{}", logs.stderr);
        }
        Commands::Exec(args) => {
            let logs = engine.exec(&args.service, &args.cmd).await?;
            print!("{}", logs.stdout);
            eprint!("{}", logs.stderr);
        }
        Commands::Config(_) => {
            println!("{}", engine.config()?);
        }
    }
    Ok(())
}
