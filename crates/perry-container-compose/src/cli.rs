use crate::error::Result;
use crate::orchestrate::Orchestrator;
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

/// perry-compose: Docker Compose-like experience for Apple Container
#[derive(Parser, Debug)]
#[command(
    name = "perry-compose",
    version,
    about = "Docker Compose-like CLI for Apple Container, powered by Perry",
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
    /// Start services (alias: start)
    Up(UpArgs),
    /// Stop and remove services (alias: down)
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

// ============ Argument structs ============

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

// ============ Command dispatch ============

pub async fn run(cli: Cli) -> Result<()> {
    let orchestrator = Orchestrator::new(
        &cli.files,
        cli.project_name.as_deref(),
        &cli.env_files,
    )?;

    match cli.command {
        Commands::Up(args) => {
            orchestrator
                .up(&args.services, args.detach, args.build)
                .await?;
        }

        Commands::Down(args) => {
            orchestrator
                .down(&args.services, args.remove_orphans, args.volumes)
                .await?;
        }

        Commands::Start(args) => {
            // `start` = up without --build (services that already have an image or container)
            orchestrator.up(&args.services, true, false).await?;
        }

        Commands::Stop(args) => {
            orchestrator.down(&args.services, false, false).await?;
        }

        Commands::Restart(args) => {
            orchestrator.down(&args.services, false, false).await?;
            orchestrator.up(&args.services, true, false).await?;
        }

        Commands::Ps(_args) => {
            let statuses = orchestrator.ps().await?;
            print_ps_table(&statuses);
        }

        Commands::Logs(args) => {
            let logs_map = orchestrator
                .logs(&args.services, args.tail, args.follow)
                .await?;

            // Print logs sorted by service name
            let mut names: Vec<&String> = logs_map.keys().collect();
            names.sort();
            for name in names {
                let log = &logs_map[name];
                if !log.is_empty() {
                    for line in log.lines() {
                        println!("{} | {}", name, line);
                    }
                }
            }
        }

        Commands::Exec(args) => {
            // Parse -e KEY=VALUE pairs
            let env: std::collections::HashMap<String, String> = args
                .env
                .iter()
                .filter_map(|e| {
                    let mut parts = e.splitn(2, '=');
                    let k = parts.next()?.to_owned();
                    let v = parts.next().unwrap_or("").to_owned();
                    Some((k, v))
                })
                .collect();

            let result = orchestrator
                .exec(
                    &args.service,
                    &args.cmd,
                    args.user.as_deref(),
                    args.workdir.as_deref(),
                    if env.is_empty() { None } else { Some(&env) },
                )
                .await?;

            print!("{}", result.stdout);
            eprint!("{}", result.stderr);

            if result.exit_code != 0 {
                std::process::exit(result.exit_code);
            }
        }

        Commands::Config(args) => {
            let yaml = orchestrator.config()?;
            if args.format == "json" {
                // Convert YAML → JSON for --format=json
                let value: serde_yaml::Value = serde_yaml::from_str(&yaml)?;
                let json = serde_json::to_string_pretty(&value)?;
                println!("{}", json);
            } else {
                println!("{}", yaml);
            }
        }
    }

    Ok(())
}

// ============ Output formatting ============

fn print_ps_table(statuses: &[crate::orchestrate::ServiceStatus]) {
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
            crate::commands::ContainerStatus::Running => "running",
            crate::commands::ContainerStatus::Stopped => "stopped",
            crate::commands::ContainerStatus::NotFound => "not found",
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
