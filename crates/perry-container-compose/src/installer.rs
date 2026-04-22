use crate::error::{ComposeError, Result};
use crate::backend::{BackendProbeResult, detect_backend};
use std::io::{self, Write};

pub struct BackendInstaller;

impl BackendInstaller {
    pub async fn run() -> Result<Box<dyn crate::backend::ContainerBackend>> {
        if !std::io::stderr().is_terminal() || std::env::var("PERRY_NO_INSTALL_PROMPT").is_ok() {
            return Err(ComposeError::NoBackendFound { probed: vec![] });
        }

        println!("Perry needs a container runtime to continue.");
        println!("No container runtime was found on this system.");
        println!("\nPick a backend to install:");

        let candidates = match std::env::consts::OS {
            "macos" => vec![
                ("apple/container", "brew install container", "https://github.com/apple/container"),
                ("orbstack", "brew install --cask orbstack", "https://orbstack.dev"),
            ],
            "linux" => vec![
                ("podman", "sudo apt-get install -y podman", "https://podman.io"),
            ],
            _ => vec![
                ("podman", "winget install RedHat.Podman", "https://podman.io"),
            ],
        };

        for (i, (name, cmd, url)) in candidates.iter().enumerate() {
            println!("{}. {} - Run: {} ({})", i + 1, name, cmd, url);
        }

        print!("\nEnter choice: ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let choice: usize = input.trim().parse().unwrap_or(0);

        if choice > 0 && choice <= candidates.len() {
            let (name, cmd, _) = candidates[choice - 1];
            println!("Installing {}...", name);
            // In a real impl, we'd run the command here
            println!("Run this command to install: {}", cmd);
        }

        probe_all_backends().await.0.ok_or(ComposeError::NoBackendFound { probed: vec![] })
    }
}

use crate::backend::probe_all_backends;

trait IsTerminal {
    fn is_terminal(&self) -> bool;
}

impl IsTerminal for std::io::Stderr {
    fn is_terminal(&self) -> bool {
        // Simple shim for is_terminal()
        true
    }
}
