use crate::error::{ComposeError, Result};
use crate::backend::{detect_backend, ContainerBackend};
use dialoguer::{Select, Confirm};
use console::{style, Term};
use std::process::Stdio;

pub struct BackendInstaller;

struct BackendOption {
    name: &'static str,
    description: &'static str,
    install_cmd: &'static str,
    docs_url: &'static str,
}

impl BackendInstaller {
    pub async fn run() -> Result<Box<dyn ContainerBackend>> {
        let term = Term::stderr();
        if !term.is_term() || std::env::var("PERRY_NO_INSTALL_PROMPT").is_ok() {
             return Err(ComposeError::NoBackendFound { probed: vec![] });
        }

        println!("\n{}", style("Perry needs a container runtime to continue.").bold());
        println!("No container runtime was found on this system.\n");

        let options = Self::get_platform_options();
        let items: Vec<String> = options.iter()
            .map(|o| format!("{} - {}", style(o.name).bold(), o.description))
            .collect();

        let selection = Select::new()
            .with_prompt("Select a backend to install")
            .items(&items)
            .default(0)
            .interact_on_opt(&term)
            .map_err(|e| ComposeError::IoError(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        if let Some(idx) = selection {
            let opt = &options[idx];
            println!("\nTo install {}, run:", style(opt.name).cyan());
            println!("  {}\n", style(opt.install_cmd).bold());
            println!("Docs: {}\n", opt.docs_url);

            if Confirm::new()
                .with_prompt(format!("Install {} now?", opt.name))
                .interact_on_opt(&term)
                .map_err(|e| ComposeError::IoError(std::io::Error::new(std::io::ErrorKind::Other, e)))?
                .unwrap_or(false)
            {
                println!("Running install command...");
                let mut child = tokio::process::Command::new("sh")
                    .arg("-c")
                    .arg(opt.install_cmd)
                    .spawn()
                    .map_err(ComposeError::IoError)?;

                let status = child.wait().await.map_err(ComposeError::IoError)?;
                if status.success() {
                    println!("{}", style("Installation successful!").green());
                    return detect_backend().await;
                } else {
                    println!("{}", style("Installation failed.").red());
                }
            }
        }

        Err(ComposeError::NoBackendFound { probed: vec![] })
    }

    fn get_platform_options() -> Vec<BackendOption> {
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            vec![
                BackendOption {
                    name: "apple/container",
                    description: "Apple's native container runtime (recommended)",
                    install_cmd: "brew install container",
                    docs_url: "https://github.com/apple/container",
                },
                BackendOption {
                    name: "podman",
                    description: "Daemonless, rootless OCI runtime",
                    install_cmd: "brew install podman && podman machine init && podman machine start",
                    docs_url: "https://podman.io",
                },
                BackendOption {
                    name: "orbstack",
                    description: "Fast macOS VM with Docker-compatible API",
                    install_cmd: "brew install --cask orbstack",
                    docs_url: "https://orbstack.dev",
                },
            ]
        }
        #[cfg(not(any(target_os = "macos", target_os = "ios")))]
        {
            vec![
                BackendOption {
                    name: "podman",
                    description: "Daemonless, rootless OCI runtime (recommended)",
                    install_cmd: "sudo apt-get install -y podman", // Simplified
                    docs_url: "https://podman.io/getting-started/installation",
                },
                BackendOption {
                    name: "docker",
                    description: "Docker Engine",
                    install_cmd: "curl -fsSL https://get.docker.com | sh",
                    docs_url: "https://docs.docker.com/engine/install",
                },
            ]
        }
    }
}
