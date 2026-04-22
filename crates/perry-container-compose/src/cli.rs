use crate::backend::ContainerBackend;
use crate::error::{ComposeError, Result};
use crate::project::ComposeProject;
use crate::compose::{ComposeEngine, ServiceStatus, ContainerStatus};
use crate::backend::detect_backend;
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;

/// perry-compose: OCI-compatible Container Compose experience
#[derive(Parser, Debug)]
#[command(
    name = "perry-compose",
    version,
    about = "OCI-compatible Container Compose CLI, powered by Perry",
    long_about = None
)]
pub struct Cli {
    /// Path to compose file(s)
    #[arg(short = 'f', long = "file", value_name = "FILE", global = true)]
    pub files: Vec<PathBuf>,

    /// Project name (default: directory name)
    #[arg(short = 'p', long = "project-name", global = true)]
    pub project_name: Option<String>,

    /// Environment file(s)
    #[arg(long = "env-file", value_name = "FILE", global = true)]
    pub env_files: Vec<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start services
    Up(UpArgs),
    /// Stop and remove services
    Down(DownArgs),
    /// Start existing stopped services
    Start(ServiceArgs),
    /// Stop running services
    Stop(ServiceArgs),
    /// Restart services
    Restart(ServiceArgs),
    /// List service status
    Ps(PsArgs),
    /// View output from containers
    Logs(LogsArgs),
    /// Execute a command in a running service
    Exec(ExecArgs),
    /// Validate and view the Compose file
    Config(ConfigArgs),
}

#[derive(Args, Debug)]
pub struct UpArgs {
    /// Start in detached mode
    #[arg(short = 'd', long = "detach")]
    pub detach: bool,
    /// Build images before starting
    #[arg(long = "build")]
    pub build: bool,
    /// Remove containers for services not in the compose file
    #[arg(long = "remove-orphans")]
    pub remove_orphans: bool,
    /// Services to start (empty = all)
    pub services: Vec<String>,
}

#[derive(Args, Debug)]
pub struct DownArgs {
    /// Remove named volumes
    #[arg(short = 'v', long = "volumes")]
    pub volumes: bool,
    /// Remove containers for services not in the compose file
    #[arg(long = "remove-orphans")]
    pub remove_orphans: bool,
    /// Services to remove (empty = all)
    pub services: Vec<String>,
}

#[derive(Args, Debug)]
pub struct ServiceArgs {
    /// Services to act on (empty = all)
    pub services: Vec<String>,
}

#[derive(Args, Debug)]
pub struct PsArgs {
    /// Show all containers (including stopped)
    #[arg(short = 'a', long = "all")]
    pub all: bool,
    /// Filter by service name
    pub services: Vec<String>,
}

#[derive(Args, Debug)]
pub struct LogsArgs {
    /// Follow log output
    #[arg(short = 'f', long = "follow")]
    pub follow: bool,
    /// Number of lines to show from the end
    #[arg(long = "tail")]
    pub tail: Option<u32>,
    /// Show timestamps
    #[arg(short = 't', long = "timestamps")]
    pub timestamps: bool,
    /// Services to show logs for (empty = all)
    pub services: Vec<String>,
}

#[derive(Args, Debug)]
pub struct ExecArgs {
    /// Service name
    pub service: String,
    /// Command to run
    pub cmd: Vec<String>,
    /// User context
    #[arg(short = 'u', long = "user")]
    pub user: Option<String>,
    /// Working directory
    #[arg(short = 'w', long = "workdir")]
    pub workdir: Option<String>,
    /// Environment variables
    #[arg(short = 'e', long = "env")]
    pub env: Vec<String>,
}

#[derive(Args, Debug)]
pub struct ConfigArgs {
    /// Output format
    #[arg(long = "format", default_value = "yaml")]
    pub format: String,
    /// Resolve environment variables
    #[arg(long = "resolve-image-digests")]
    pub resolve: bool,
}

pub async fn run(cli: Cli) -> Result<()> {
    let backend = detect_backend().await.map_err(|probed| {
        ComposeError::NoBackendFound { probed }
    })?;

    let project = ComposeProject::load(&cli.files, cli.project_name, &cli.env_files)?;
    let engine = ComposeEngine::new(
        project.spec.clone(),
        project.project_name.clone(),
        Arc::from(backend as Box<dyn ContainerBackend>),
    );

    match cli.command {
        Commands::Up(args) => {
            engine.up(&args.services, args.detach, args.build, args.remove_orphans).await?;
        }
        Commands::Down(args) => {
            engine.down(args.volumes, args.remove_orphans).await?;
        }
        Commands::Start(args) => {
            engine.start(&args.services).await?;
        }
        Commands::Stop(args) => {
            engine.stop(&args.services).await?;
        }
        Commands::Restart(args) => {
            engine.restart(&args.services).await?;
        }
        Commands::Ps(_args) => {
            let info_list = engine.ps().await?;
            let status_list: Vec<ServiceStatus> = info_list.into_iter().map(|i| ServiceStatus {
                service_name: i.name.clone(),
                container_name: i.name,
                status: ContainerStatus::Running,
            }).collect();
            print_ps_table(&status_list);
        }
        Commands::Logs(args) => {
            let logs = engine.logs(args.services.first().map(|s| s.as_str()), args.tail).await?;
            print!("{}", logs.stdout);
            eprint!("{}", logs.stderr);
        }
        Commands::Exec(args) => {
            let logs = engine.exec(&args.service, &args.cmd).await?;
            print!("{}", logs.stdout);
            eprint!("{}", logs.stderr);
        }
        Commands::Config(args) => {
            if args.format == "json" {
                println!("{}", serde_json::to_string_pretty(&project.spec).unwrap_or_default());
            } else {
                // In a real implementation we would re-serialize to YAML
                println!("Resolved configuration for project: {}", project.project_dir.display());
            }
        }
    }

    Ok(())
}

fn print_ps_table(statuses: &[ServiceStatus]) {
    let col_w_svc = 24usize;
    let col_w_status = 12usize;
    let col_w_container = 36usize;

    println!(
        "{:<col_w_svc$}  {:<col_w_status$}  {:<col_w_container$}",
        "SERVICE", "STATUS", "CONTAINER",
        col_w_svc = col_w_svc,
        col_w_status = col_w_status,
        col_w_container = col_w_container,
    );
    println!("{}", "-".repeat(col_w_svc + col_w_status + col_w_container + 4));

    for s in statuses {
        let status_str = match s.status {
            ContainerStatus::Running => "running",
            ContainerStatus::Stopped => "stopped",
            ContainerStatus::NotFound => "not found",
        };
        println!(
            "{:<col_w_svc$}  {:<col_w_status$}  {:<col_w_container$}",
            s.service_name,
            status_str,
            s.container_name,
            col_w_svc = col_w_svc,
            col_w_status = col_w_status,
            col_w_container = col_w_container,
        );
    }
}
