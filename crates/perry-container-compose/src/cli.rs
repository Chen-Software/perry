use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "perry-container-compose")]
#[command(about = "Native Rust reimplementation of container-compose", long_about = None)]
pub struct Cli {
    #[arg(short, long, value_name = "FILE")]
    pub file: Vec<PathBuf>,

    #[arg(short, long)]
    pub project_name: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Up {
        #[arg(short, long)]
        detach: bool,
    },
    Down {
        #[arg(short, long)]
        volumes: bool,
    },
    Ps,
    Logs {
        #[arg(short, long)]
        follow: bool,
        #[arg(long)]
        tail: Option<u32>,
        services: Vec<String>,
    },
}
