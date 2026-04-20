use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "perry-compose")]
#[command(version = "1.0")]
#[command(about = "Native OCI multi-container orchestration", long_about = None)]
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
    Start { services: Vec<String> },
    Stop { services: Vec<String> },
    Restart { services: Vec<String> },
}
