use crate::backend::{BackendDriver, probe_driver};
use crate::error::{ComposeError, Result};
use std::process::Command;

pub struct BackendInstaller;

impl BackendInstaller {
    #[cfg(feature = "installer")]
    pub async fn run() -> Result<BackendDriver> {
        use dialoguer::{Select, theme::ColorfulTheme};
        use console::Term;

        if !Term::stderr().is_term() || std::env::var("PERRY_NO_INSTALL_PROMPT").is_ok() {
            return Err(ComposeError::validation("Non-interactive mode: cannot prompt for backend installation"));
        }

        println!("No container backend found. Perry requires an OCI runtime.");

        let options = if cfg!(target_os = "macos") {
            vec![
                ("apple/container", "Direct from App Store (Native)"),
                ("orbstack", "brew install orbstack (Recommended)"),
                ("colima", "brew install colima"),
                ("podman", "brew install podman"),
            ]
        } else if cfg!(target_os = "linux") {
            vec![
                ("podman", "sudo apt install podman (Recommended)"),
                ("docker", "Follow docker.com instructions"),
            ]
        } else {
            vec![
                ("podman", "Follow podman.io instructions"),
                ("docker", "Follow docker.com instructions"),
            ]
        };

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select a backend to install")
            .items(&options.iter().map(|o| format!("{} - {}", o.0, o.1)).collect::<Vec<_>>())
            .default(0)
            .interact()
            .map_err(|e| ComposeError::validation(e.to_string()))?;

        let (name, _) = options[selection];

        let install_cmd = match name {
            "orbstack" => "brew install --cask orbstack",
            "colima" => "brew install colima",
            "podman" => if cfg!(target_os = "macos") { "brew install podman" } else { "sudo apt install -y podman" },
            _ => return Err(ComposeError::validation(format!("Please install {} manually.", name))),
        };

        println!("Running: {}", install_cmd);
        let mut child = if cfg!(target_os = "windows") {
            Command::new("cmd").args(["/C", install_cmd]).spawn()?
        } else {
            Command::new("sh").args(["-c", install_cmd]).spawn()?
        };

        let status = child.wait()?;
        if !status.success() {
            return Err(ComposeError::validation("Installation failed"));
        }

        // Post-install setup
        if name == "podman" && cfg!(target_os = "macos") {
            println!("Initializing podman machine...");
            let _ = Command::new("podman").args(["machine", "init"]).status();
            let _ = Command::new("podman").args(["machine", "start"]).status();
        }

        println!("Verifying installation...");
        probe_driver(name).await.map_err(|e| ComposeError::validation(format!("Verification failed: {}", e)))
    }

    #[cfg(not(feature = "installer"))]
    pub async fn run() -> Result<BackendDriver> {
        Err(ComposeError::validation("Backend installer not available in this build. Install a backend (e.g. apple/container, podman) manually."))
    }
}
