//! Interactive backend installer for perry-container-compose.

use crate::backend::{detect_backend, ContainerBackend};
use crate::error::{ComposeError, Result};
use std::sync::Arc;
use console::{style, Term};
use dialoguer::{theme::ColorfulTheme, Select, Confirm};

pub struct BackendInstaller {
    pub no_prompt: bool,
}

struct InstallOption {
    name: &'static str,
    description: &'static str,
    install_command: &'static str,
    docs_url: &'static str,
}

impl BackendInstaller {
    pub fn new() -> Self {
        let no_prompt = std::env::var("PERRY_NO_INSTALL_PROMPT").is_ok();
        Self { no_prompt }
    }

    pub async fn run(&self) -> Result<Arc<dyn ContainerBackend + Send + Sync>> {
        if self.no_prompt {
            return Err(ComposeError::validation("No container backend found and PERRY_NO_INSTALL_PROMPT is set."));
        }

        if !Term::stderr().is_term() {
            return Err(ComposeError::validation("No container backend found and stderr is not a TTY."));
        }

        println!("{}", style("Perry needs a container runtime to continue.").bold());
        println!("No container runtime was found on this system.");
        println!();

        let options = self.platform_options();
        let items: Vec<String> = options.iter()
            .map(|o| format!("{} - {}", style(o.name).bold(), o.description))
            .collect();

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select a backend to install")
            .items(&items)
            .default(0)
            .interact()
            .map_err(|e| ComposeError::validation(format!("Selection failed: {}", e)))?;

        let choice = &options[selection];

        println!();
        println!("To install {}, run:", style(choice.name).cyan());
        println!("  {}", style(choice.install_command).bold());
        println!("Docs: {}", style(choice.docs_url).underlined());
        println!();

        if Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(format!("Run install command automatically?"))
            .interact()
            .unwrap_or(false)
        {
            self.execute_install(choice.install_command).await?;

            println!("{}", style("Installation completed. Verifying...").green());
            match detect_backend().await {
                Ok(backend) => Ok(backend),
                Err(_) => Err(ComposeError::validation("Installation finished but backend still not detected. Please install manually.")),
            }
        } else {
            Err(ComposeError::validation("Please install the container runtime and try again."))
        }
    }

    fn platform_options(&self) -> Vec<InstallOption> {
        if cfg!(target_os = "macos") || cfg!(target_os = "ios") {
            vec![
                InstallOption {
                    name: "apple/container",
                    description: "Apple's native container runtime (recommended)",
                    install_command: "brew install container",
                    docs_url: "https://github.com/apple/container",
                },
                InstallOption {
                    name: "orbstack",
                    description: "Fast macOS VM with Docker-compatible API",
                    install_command: "brew install --cask orbstack",
                    docs_url: "https://orbstack.dev",
                },
                InstallOption {
                    name: "colima",
                    description: "Lightweight macOS container runtime",
                    install_command: "brew install colima",
                    docs_url: "https://github.com/abiosoft/colima",
                },
                InstallOption {
                    name: "podman",
                    description: "Daemonless, rootless OCI runtime",
                    install_command: "brew install podman && podman machine init && podman machine start",
                    docs_url: "https://podman.io",
                },
                InstallOption {
                    name: "docker",
                    description: "Docker Desktop for Mac",
                    install_command: "brew install --cask docker",
                    docs_url: "https://docs.docker.com/desktop/mac",
                },
            ]
        } else if cfg!(target_os = "linux") {
            vec![
                InstallOption {
                    name: "podman",
                    description: "Daemonless, rootless OCI runtime (recommended)",
                    install_command: "sudo apt-get install -y podman", // Simplified; ideally would detect distro
                    docs_url: "https://podman.io/getting-started/installation",
                },
                InstallOption {
                    name: "nerdctl",
                    description: "containerd CLI wrapper",
                    install_command: "brew install nerdctl", // Or other methods
                    docs_url: "https://github.com/containerd/nerdctl",
                },
                InstallOption {
                    name: "docker",
                    description: "Docker Engine",
                    install_command: "curl -fsSL https://get.docker.com | sh",
                    docs_url: "https://docs.docker.com/engine/install",
                },
            ]
        } else if cfg!(target_os = "windows") {
            vec![
                InstallOption {
                    name: "podman",
                    description: "Daemonless, rootless OCI runtime (recommended)",
                    install_command: "winget install RedHat.Podman",
                    docs_url: "https://podman.io/getting-started/installation",
                },
                InstallOption {
                    name: "docker",
                    description: "Docker Desktop for Windows",
                    install_command: "winget install Docker.DockerDesktop",
                    docs_url: "https://docs.docker.com/desktop/windows",
                },
            ]
        } else {
            vec![]
        }
    }

    async fn execute_install(&self, command: &str) -> Result<()> {
        let status = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .status()
            .await
            .map_err(ComposeError::IoError)?;

        if status.success() {
            Ok(())
        } else {
            Err(ComposeError::validation(format!("Install command failed with status: {}", status)))
        }
    }
}
