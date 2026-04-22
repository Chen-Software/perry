use crate::backend::{detect_backend, BackendDriver};
use crate::error::{ComposeError, Result};
use dialoguer::Select;
use console::{style, Term};
use std::path::PathBuf;

pub struct BackendInstaller {
    pub is_tty: bool,
}

impl BackendInstaller {
    pub fn new() -> Self {
        Self { is_tty: Term::stderr().is_term() }
    }

    pub async fn run(&self) -> Result<Box<dyn crate::backend::ContainerBackend>> {
        if !self.is_tty || std::env::var("PERRY_NO_INSTALL_PROMPT").is_ok() {
             return Err(ComposeError::NoBackendFound { probed: vec![] });
        }

        println!("{}", style("Perry needs a container runtime to continue.").bold());
        println!("No container runtime was found on this system.\n");

        let candidates = self.get_install_candidates();
        let items: Vec<String> = candidates.iter().map(|c| format!("{} - {}", style(&c.name).bold(), c.description)).collect();

        let selection = Select::new()
            .with_prompt("Select a backend to install")
            .items(&items)
            .default(0)
            .interact()
            .map_err(|_| ComposeError::NoBackendFound { probed: vec![] })?;

        let choice = &candidates[selection];
        println!("\nTo install {}, run:", style(&choice.name).cyan());
        println!("  {}\n", style(&choice.install_cmd).cyan());
        println!("Docs: {}\n", choice.docs_url);

        // For MVP, we don't execute automatically yet, just return the error with instructions.
        // Or we can try to execute.
        Err(ComposeError::NoBackendFound { probed: vec![] })
    }

    fn get_install_candidates(&self) -> Vec<InstallCandidate> {
        match std::env::consts::OS {
            "macos" | "ios" => vec![
                InstallCandidate {
                    name: "apple/container".into(),
                    description: "Apple's native container runtime (recommended)".into(),
                    install_cmd: "brew install container".into(),
                    docs_url: "https://github.com/apple/container".into(),
                },
                InstallCandidate {
                    name: "orbstack".into(),
                    description: "Fast macOS VM with Docker-compatible API".into(),
                    install_cmd: "brew install --cask orbstack".into(),
                    docs_url: "https://orbstack.dev".into(),
                },
            ],
            _ => vec![
                 InstallCandidate {
                    name: "podman".into(),
                    description: "Daemonless, rootless OCI runtime (recommended)".into(),
                    install_cmd: "sudo apt-get install -y podman".into(),
                    docs_url: "https://podman.io".into(),
                },
            ]
        }
    }
}

struct InstallCandidate {
    name: String,
    description: String,
    install_cmd: String,
    docs_url: String,
}
