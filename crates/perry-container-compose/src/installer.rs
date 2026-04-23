use crate::error::{Result, ComposeError};
use crate::backend::{BackendDriver, detect_backend, platform_candidates};
use tokio::process::Command;

pub struct BackendInstaller;

#[derive(Clone)]
struct InstallerOption {
    name: &'static str,
    description: &'static str,
    install_cmd: &'static str,
    docs_url: &'static str,
}

impl BackendInstaller {
    pub async fn run() -> Result<(BackendDriver, Box<dyn crate::backend::ContainerBackend>)> {
        #[cfg(not(feature = "installer"))]
        {
            return Err(ComposeError::NoBackendFound { probed: vec![] });
        }

        #[cfg(feature = "installer")]
        {
            use console::{style, Term};
            use dialoguer::{Select, theme::ColorfulTheme};

            if !Term::stderr().is_term() || std::env::var("PERRY_NO_INSTALL_PROMPT").is_ok() {
                return Err(ComposeError::NoBackendFound { probed: vec![] });
            }

            println!("\n{}", style("Perry needs a container runtime to continue. No container runtime was found on this system.").bold());

            let options = Self::get_options();
            let items: Vec<String> = options.iter().map(|o| format!("{} - {}", style(o.name).bold(), o.description)).collect();

            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Select a backend to install")
                .items(&items)
                .default(0)
                .interact_opt()
                .map_err(|e| ComposeError::IoError(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

            if let Some(index) = selection {
                let option = &options[index];
                println!("\nInstall {} now?", style(option.name).bold());
                println!("  Run: {}", style(option.install_cmd).cyan());
                println!("  Docs: {}\n", style(option.docs_url).underlined());

                let confirm = dialoguer::Confirm::with_theme(&ColorfulTheme::default())
                    .with_prompt("Do you want to proceed?")
                    .interact()
                    .map_err(|e| ComposeError::IoError(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

                if confirm {
                    let mut cmd = if cfg!(windows) {
                        let mut c = Command::new("powershell");
                        c.args(["-Command", option.install_cmd]);
                        c
                    } else {
                        let mut c = Command::new("sh");
                        c.args(["-c", option.install_cmd]);
                        c
                    };

                    let status = cmd.status().await.map_err(ComposeError::IoError)?;
                    if status.success() {
                        println!("\n{}", style("Installation successful!").green().bold());
                        detect_backend().await.map_err(|p| ComposeError::NoBackendFound { probed: p })
                    } else {
                        println!("\n{}", style("Installation failed.").red().bold());
                        Err(ComposeError::NoBackendFound { probed: vec![] })
                    }
                } else {
                    Err(ComposeError::NoBackendFound { probed: vec![] })
                }
            } else {
                Err(ComposeError::NoBackendFound { probed: vec![] })
            }
        }
    }

    fn get_options() -> Vec<InstallerOption> {
        match std::env::consts::OS {
            "macos" | "ios" => vec![
                InstallerOption {
                    name: "apple/container",
                    description: "Apple's native container runtime (recommended)",
                    install_cmd: "brew install container",
                    docs_url: "https://github.com/apple/container",
                },
                InstallerOption {
                    name: "orbstack",
                    description: "Fast macOS VM with Docker-compatible API",
                    install_cmd: "brew install --cask orbstack",
                    docs_url: "https://orbstack.dev",
                },
                InstallerOption {
                    name: "colima",
                    description: "Lightweight macOS container runtime",
                    install_cmd: "brew install colima",
                    docs_url: "https://github.com/abiosoft/colima",
                },
                InstallerOption {
                    name: "podman",
                    description: "Daemonless, rootless OCI runtime",
                    install_cmd: "brew install podman && podman machine init && podman machine start",
                    docs_url: "https://podman.io",
                },
                InstallerOption {
                    name: "docker",
                    description: "Docker Desktop for Mac",
                    install_cmd: "brew install --cask docker",
                    docs_url: "https://docs.docker.com/desktop/mac",
                },
            ],
            "linux" => vec![
                InstallerOption {
                    name: "podman",
                    description: "Daemonless, rootless OCI runtime (recommended)",
                    install_cmd: "sudo apt-get install -y podman || sudo dnf install -y podman",
                    docs_url: "https://podman.io/getting-started/installation",
                },
                InstallerOption {
                    name: "nerdctl",
                    description: "containerd CLI wrapper",
                    install_cmd: "brew install nerdctl",
                    docs_url: "https://github.com/containerd/nerdctl",
                },
                InstallerOption {
                    name: "docker",
                    description: "Docker Engine",
                    install_cmd: "curl -fsSL https://get.docker.com | sh",
                    docs_url: "https://docs.docker.com/engine/install",
                },
            ],
            "windows" => vec![
                InstallerOption {
                    name: "podman",
                    description: "Daemonless, rootless OCI runtime (recommended)",
                    install_cmd: "winget install RedHat.Podman",
                    docs_url: "https://podman.io/getting-started/installation",
                },
                InstallerOption {
                    name: "docker",
                    description: "Docker Desktop for Windows",
                    install_cmd: "winget install Docker.DockerDesktop",
                    docs_url: "https://docs.docker.com/desktop/windows",
                },
            ],
            _ => vec![],
        }
    }
}
