use crate::error::{ComposeError, Result, BackendProbeResult};
use crate::backend::{detect_backend, BackendDriver, ContainerBackend};
use std::process::Command;
use std::sync::Arc;

#[cfg(feature = "installer")]
use dialoguer::{theme::ColorfulTheme, Select};
#[cfg(feature = "installer")]
use console::{style, Term};

pub struct BackendInstaller {
    pub probed: Vec<BackendProbeResult>,
}

impl BackendInstaller {
    pub async fn run(&self) -> Result<Arc<dyn ContainerBackend + Send + Sync>> {
        #[cfg(not(feature = "installer"))]
        {
            return Err(ComposeError::NoBackendFound { probed: self.probed.clone() });
        }

        #[cfg(feature = "installer")]
        {
            if !Term::stderr().is_term() || std::env::var("PERRY_NO_INSTALL_PROMPT").is_ok() {
                return Err(ComposeError::NoBackendFound { probed: self.probed.clone() });
            }

            println!("{}", style("Perry needs a container runtime to continue.").bold());
            println!("No container runtime was found on this system.\n");

            let backends = self.get_installable_backends();
            let items: Vec<String> = backends.iter()
                .map(|b| format!("{} - {}", style(&b.name).bold(), b.description))
                .collect();

            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Select a backend to install")
                .items(&items)
                .default(0)
                .interact_opt()
                .map_err(|e| ComposeError::IoError(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

            if let Some(index) = selection {
                let selected = &backends[index];
                println!("\nTo install {}, run:", style(&selected.name).cyan());
                println!("  {}\n", style(&selected.command).cyan());
                println!("More info: {}\n", style(&selected.docs).underlined());

                print!("Install {} now? [y/N]: ", selected.name);
                use std::io::Write;
                std::io::stdout().flush().ok();

                let mut input = String::new();
                std::io::stdin().read_line(&mut input).ok();

                if input.trim().to_lowercase() == "y" {
                    println!("Running: {}", selected.command);
                    let status = if cfg!(target_os = "windows") {
                        Command::new("powershell").args(["-Command", &selected.command]).status()
                    } else {
                        Command::new("sh").args(["-c", &selected.command]).status()
                    }.map_err(ComposeError::IoError)?;

                    if status.success() {
                        println!("{}", style("Installation successful!").green());
                        match detect_backend().await {
                            Ok(b) => Ok(b),
                            Err(e) => Err(ComposeError::NoBackendFound { probed: e }),
                        }
                    } else {
                        println!("{}", style("Installation failed.").red());
                        Err(ComposeError::NoBackendFound { probed: self.probed.clone() })
                    }
                } else {
                    Err(ComposeError::NoBackendFound { probed: self.probed.clone() })
                }
            } else {
                Err(ComposeError::NoBackendFound { probed: self.probed.clone() })
            }
        }
    }

    fn get_installable_backends(&self) -> Vec<InstallableBackend> {
        match std::env::consts::OS {
            "macos" | "ios" => vec![
                InstallableBackend {
                    name: "apple/container".into(),
                    description: "Apple's native container runtime (recommended)".into(),
                    command: "brew install container".into(),
                    docs: "https://github.com/apple/container".into(),
                },
                InstallableBackend {
                    name: "orbstack".into(),
                    description: "Fast macOS VM with Docker-compatible API".into(),
                    command: "brew install --cask orbstack".into(),
                    docs: "https://orbstack.dev".into(),
                },
                InstallableBackend {
                    name: "colima".into(),
                    description: "Lightweight macOS container runtime".into(),
                    command: "brew install colima".into(),
                    docs: "https://github.com/abiosoft/colima".into(),
                },
                InstallableBackend {
                    name: "podman".into(),
                    description: "Daemonless, rootless OCI runtime".into(),
                    command: "brew install podman && podman machine init && podman machine start".into(),
                    docs: "https://podman.io".into(),
                },
                InstallableBackend {
                    name: "docker".into(),
                    description: "Docker Desktop for Mac".into(),
                    command: "brew install --cask docker".into(),
                    docs: "https://docs.docker.com/desktop/mac".into(),
                },
            ],
            "linux" => vec![
                InstallableBackend {
                    name: "podman".into(),
                    description: "Daemonless, rootless OCI runtime (recommended)".into(),
                    command: "sudo apt-get install -y podman".into(),
                    docs: "https://podman.io/getting-started/installation".into(),
                },
                InstallableBackend {
                    name: "docker".into(),
                    description: "Docker Engine".into(),
                    command: "curl -fsSL https://get.docker.com | sh".into(),
                    docs: "https://docs.docker.com/engine/install".into(),
                },
            ],
            "windows" => vec![
                InstallableBackend {
                    name: "podman".into(),
                    description: "Daemonless, rootless OCI runtime (recommended)".into(),
                    command: "winget install RedHat.Podman".into(),
                    docs: "https://podman.io/getting-started/installation".into(),
                },
                InstallableBackend {
                    name: "docker".into(),
                    description: "Docker Desktop for Windows".into(),
                    command: "winget install Docker.DockerDesktop".into(),
                    docs: "https://docs.docker.com/desktop/windows".into(),
                },
            ],
            _ => vec![],
        }
    }
}

struct InstallableBackend {
    name: String,
    description: String,
    command: String,
    docs: String,
}
