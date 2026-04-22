use crate::error::{ComposeError, Result};
use crate::backend::{detect_backend, platform_candidates, CliBackend};

pub struct BackendInstaller;

impl BackendInstaller {
    pub async fn run() -> Result<CliBackend> {
        #[cfg(feature = "installer")]
        {
            use console::{style, Term};
            use dialoguer::{theme::ColorfulTheme, Select};

            if !Term::stderr().is_term() || std::env::var("PERRY_NO_INSTALL_PROMPT").is_ok() {
                return Err(ComposeError::NoBackendFound { probed: vec![] });
            }

            println!("{}", style("Perry needs a container runtime to continue. No container runtime was found on this system.").bold());
            println!("\nSelect a backend to install:");

            let candidates = platform_candidates();
            let items: Vec<String> = candidates.iter().map(|c| c.to_string()).collect();

            let selection = Select::with_theme(&ColorfulTheme::default())
                .items(&items)
                .default(0)
                .interact_opt()
                .map_err(|e| ComposeError::IoError(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

            if let Some(index) = selection {
                let name = candidates[index];
                println!("Installation instructions for {}:", style(name).cyan());

                let (cmd_str, docs_url) = match name {
                    "apple/container" => ("brew install container", "https://github.com/apple/container"),
                    "podman" => ("brew install podman && podman machine init && podman machine start", "https://podman.io"),
                    "orbstack" => ("brew install --cask orbstack", "https://orbstack.dev"),
                    "colima" => ("brew install colima", "https://github.com/abiosoft/colima"),
                    "docker" => ("brew install --cask docker", "https://docs.docker.com/desktop/mac"),
                    _ => ("", ""),
                };

                if !cmd_str.is_empty() {
                    println!("Run: {}\nDocs: {}", style(cmd_str).cyan(), docs_url);
                } else {
                    println!("Please refer to the documentation for {}", name);
                }

                println!("\nInstall now? [y/N]");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input).ok();
                if input.trim().to_lowercase() == "y" && !cmd_str.is_empty() {
                    let parts: Vec<&str> = cmd_str.split("&&").map(|s| s.trim()).collect();
                    for part in parts {
                        let mut args: Vec<&str> = part.split_whitespace().collect();
                        let cmd = args.remove(0);
                        let status = tokio::process::Command::new(cmd)
                            .args(args)
                            .status()
                            .await
                            .map_err(|e| ComposeError::IoError(e))?;

                        if !status.success() {
                            return Err(ComposeError::BackendError {
                                code: status.code().unwrap_or(-1),
                                message: format!("Installation command failed: {}", part)
                            });
                        }
                    }
                    return detect_backend().await.map_err(|probed| ComposeError::NoBackendFound { probed });
                }
            }
        }

        Err(ComposeError::NoBackendFound { probed: vec![] })
    }
}
