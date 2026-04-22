use crate::error::{ComposeError, Result};
use crate::backend::BackendDriver;

pub struct BackendInstaller;

impl BackendInstaller {
    pub async fn run() -> Result<BackendDriver> {
        if !console::Term::stderr().is_term() {
            return Err(ComposeError::ValidationError {
                message: "No container backend found and session is non-interactive. Please install Podman or Apple Container.".to_string(),
            });
        }

        if std::env::var("PERRY_NO_INSTALL_PROMPT").is_ok() {
            return Err(ComposeError::ValidationError {
                message: "No container backend found and PERRY_NO_INSTALL_PROMPT is set.".to_string(),
            });
        }

        println!("\n🚀 No OCI container runtime (Apple Container, Podman, OrbStack) detected.");
        println!("Perry requires a container runtime to run workloads and services.\n");

        let candidates = Self::get_installable_candidates();
        if candidates.is_empty() {
            return Err(ComposeError::ValidationError {
                message: "No installable container backends found for this platform.".to_string(),
            });
        }

        let items: Vec<String> = candidates.iter().map(|(name, desc, _)| format!("{}: {}", name, desc)).collect();

        let selection = dialoguer::Select::new()
            .with_prompt("Select a container runtime to install")
            .items(&items)
            .default(0)
            .interact()
            .map_err(|e| ComposeError::ValidationError { message: e.to_string() })?;

        let (name, _, install_cmd) = &candidates[selection];

        println!("\n🛠️  Installing {}...", name);
        println!("Running: {}\n", install_cmd);

        let confirm = dialoguer::Confirm::new()
            .with_prompt(format!("Continue with installation of {}?", name))
            .default(true)
            .interact()
            .map_err(|e| ComposeError::ValidationError { message: e.to_string() })?;

        if !confirm {
            return Err(ComposeError::ValidationError {
                message: "Installation cancelled by user.".to_string(),
            });
        }

        let status = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(install_cmd)
            .status()
            .await
            .map_err(ComposeError::IoError)?;

        if !status.success() {
            return Err(ComposeError::BackendError {
                code: status.code().unwrap_or(-1),
                message: format!("Failed to install {}", name),
            });
        }

        println!("\n✅ {} installed successfully. Re-verifying...", name);

        // Re-probe for the newly installed backend
        match name {
            &"apple/container" => {
                if let Ok(bin) = which::which("container") {
                    return Ok(BackendDriver::AppleContainer { bin });
                }
            }
            &"podman" => {
                if let Ok(bin) = which::which("podman") {
                    return Ok(BackendDriver::Podman { bin });
                }
            }
            &"orbstack" => {
                if let Ok(bin) = which::which("orb") {
                    return Ok(BackendDriver::Orbstack { bin });
                }
            }
            &"colima" => {
                if let Ok(bin) = which::which("colima") {
                    return Ok(BackendDriver::Colima { bin });
                }
            }
            _ => {}
        }

        Err(ComposeError::ValidationError {
            message: format!("{} was installed but could not be located on PATH.", name),
        })
    }

    fn get_installable_candidates() -> Vec<(&'static str, &'static str, &'static str)> {
        if cfg!(target_os = "macos") {
            vec![
                ("apple/container", "Apple's native container runtime (Recommended)", "open 'https://apps.apple.com/app/apple-container'"),
                ("podman", "Daemonless OCI container engine", "brew install podman && podman machine init && podman machine start"),
                ("orbstack", "Fast, light, and easy way to run containers", "brew install orbstack"),
                ("colima", "Container runtimes on macOS with minimal setup", "brew install colima && colima start"),
            ]
        } else if cfg!(target_os = "linux") {
            vec![
                ("podman", "Daemonless OCI container engine (Recommended)", "sudo apt-get update && sudo apt-get install -y podman || sudo yum install -y podman"),
                ("docker", "The industry-standard container engine", "curl -fsSL https://get.docker.com | sh"),
            ]
        } else {
            vec![]
        }
    }
}
