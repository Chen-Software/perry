#[cfg(feature = "installer")]
use dialoguer::{theme::ColorfulTheme, Select};
#[cfg(feature = "installer")]
use console::{style, Term};
use crate::error::{ComposeError, Result};
use crate::backend::{detect_backend, ContainerBackend};

pub struct BackendInstaller {
    platform: String,
}

impl BackendInstaller {
    pub fn new() -> Self {
        Self {
            platform: std::env::consts::OS.to_string(),
        }
    }

    pub async fn run(&self) -> Result<Box<dyn ContainerBackend>> {
        #[cfg(not(feature = "installer"))]
        {
            return Err(ComposeError::NoBackendFound { probed: vec![] });
        }

        #[cfg(feature = "installer")]
        {
            if !Term::stderr().is_term() || std::env::var("PERRY_NO_INSTALL_PROMPT").is_ok() {
                 return Err(ComposeError::NoBackendFound { probed: vec![] });
            }

            println!("{}", style("Perry needs a container runtime to continue.").bold());
            println!("No container runtime was found on this system.\n");

            let backends = self.get_platform_backends();
            let items: Vec<String> = backends.iter()
                .map(|b| format!("{:<15} {}", style(&b.name).bold(), b.description))
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
                println!("  {}\n", style(&selected.install_cmd).yellow());
                println!("Docs: {}\n", style(&selected.docs_url).underlined());

                print!("Would you like to run the installation command now? [y/N]: ");
                use std::io::{Write, stdin};
                std::io::stdout().flush().unwrap();
                let mut input = String::new();
                stdin().read_line(&mut input).unwrap();

                if input.trim().to_lowercase() == "y" {
                    println!("Running: {}", selected.install_cmd);
                    let mut child = if self.platform == "windows" {
                        tokio::process::Command::new("powershell")
                            .args(["-Command", &selected.install_cmd])
                            .spawn()
                            .map_err(ComposeError::IoError)?
                    } else {
                        tokio::process::Command::new("sh")
                            .args(["-c", &selected.install_cmd])
                            .spawn()
                            .map_err(ComposeError::IoError)?
                    };

                    let status = child.wait().await.map_err(ComposeError::IoError)?;
                    if status.success() {
                        println!("{}", style("Installation successful!").green());
                        // Re-probe
                        return detect_backend().await;
                    } else {
                        println!("{}", style("Installation failed.").red());
                    }
                }
            }

            Err(ComposeError::NoBackendFound { probed: vec![] })
        }
    }

    fn get_platform_backends(&self) -> Vec<InstallableBackend> {
        match self.platform.as_str() {
            "macos" | "ios" => vec![
                InstallableBackend {
                    name: "apple/container".into(),
                    description: "Apple's native container runtime (recommended)".into(),
                    install_cmd: "brew install container".into(),
                    docs_url: "https://github.com/apple/container".into(),
                },
                InstallableBackend {
                    name: "orbstack".into(),
                    description: "Fast macOS VM with Docker-compatible API".into(),
                    install_cmd: "brew install --cask orbstack".into(),
                    docs_url: "https://orbstack.dev".into(),
                },
                InstallableBackend {
                    name: "colima".into(),
                    description: "Lightweight macOS container runtime".into(),
                    install_cmd: "brew install colima".into(),
                    docs_url: "https://github.com/abiosoft/colima".into(),
                },
                InstallableBackend {
                    name: "podman".into(),
                    description: "Daemonless, rootless OCI runtime".into(),
                    install_cmd: "brew install podman && podman machine init && podman machine start".into(),
                    docs_url: "https://podman.io".into(),
                },
                InstallableBackend {
                    name: "docker".into(),
                    description: "Docker Desktop for Mac".into(),
                    install_cmd: "brew install --cask docker".into(),
                    docs_url: "https://docs.docker.com/desktop/mac".into(),
                },
            ],
            "linux" => vec![
                InstallableBackend {
                    name: "podman".into(),
                    description: "Daemonless, rootless OCI runtime (recommended)".into(),
                    install_cmd: "sudo apt-get install -y podman".into(),
                    docs_url: "https://podman.io/getting-started/installation".into(),
                },
                InstallableBackend {
                    name: "nerdctl".into(),
                    description: "containerd CLI wrapper".into(),
                    install_cmd: "brew install nerdctl".into(),
                    docs_url: "https://github.com/containerd/nerdctl".into(),
                },
                InstallableBackend {
                    name: "docker".into(),
                    description: "Docker Engine".into(),
                    install_cmd: "curl -fsSL https://get.docker.com | sh".into(),
                    docs_url: "https://docs.docker.com/engine/install".into(),
                },
            ],
            "windows" => vec![
                InstallableBackend {
                    name: "podman".into(),
                    description: "Daemonless, rootless OCI runtime (recommended)".into(),
                    install_cmd: "winget install RedHat.Podman".into(),
                    docs_url: "https://podman.io/getting-started/installation".into(),
                },
                InstallableBackend {
                    name: "docker".into(),
                    description: "Docker Desktop for Windows".into(),
                    install_cmd: "winget install Docker.DockerDesktop".into(),
                    docs_url: "https://docs.docker.com/desktop/windows".into(),
                },
            ],
            _ => vec![],
        }
    }
}

struct InstallableBackend {
    name: String,
    description: String,
    install_cmd: String,
    docs_url: String,
}
