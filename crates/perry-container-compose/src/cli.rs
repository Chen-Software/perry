use clap::{Parser, Subcommand};
use crate::error::ComposeError;

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[arg(short, long)]
    pub file: Vec<String>,

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
    },
    Exec {
        service: String,
        command: Vec<String>,
    },
    Config,
}
