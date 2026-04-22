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

                // Simplified for brevity, in a real implementation we would have
                // a map of name -> (command, docs_url)
                match name {
                    "apple/container" => println!("Run: brew install container\nDocs: https://github.com/apple/container"),
                    "podman" => println!("Run: brew install podman && podman machine init && podman machine start\nDocs: https://podman.io"),
                    _ => println!("Please refer to the documentation for {}", name),
                }

                println!("\nInstall now? [y/N]");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input).ok();
                if input.trim().to_lowercase() == "y" {
                    // Logic to execute install command would go here
                    return detect_backend().await.map_err(|probed| ComposeError::NoBackendFound { probed });
                }
            }
        }

        Err(ComposeError::NoBackendFound { probed: vec![] })
    }
}
