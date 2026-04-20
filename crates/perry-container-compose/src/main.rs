use clap::Parser;
use perry_container_compose::cli::{Cli, Commands};
use perry_container_compose::project::ComposeProject;
use perry_container_compose::config::ProjectConfig;
use perry_container_compose::compose::ComposeEngine;
use perry_container_compose::backend::detect_backend;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let config = ProjectConfig {
        files: cli.file,
        project_name: cli.project_name,
        env_files: cli.env_file,
    };

    let project = ComposeProject::load(&config)?;
    let backend = detect_backend().await
        .map_err(|probed| anyhow::anyhow!("No container backend found. Probed: {:?}", probed))?;

    let engine = ComposeEngine::new(project.spec, project.project_name, Arc::from(backend));

    match cli.command {
        Commands::Up { detach, build, remove_orphans, services } => {
            engine.up(&services, detach, build, remove_orphans).await?;
        }
        Commands::Down { volumes, remove_orphans, services } => {
            engine.down(volumes, remove_orphans).await?;
        }
        Commands::Ps { all: _, services } => {
            let results = engine.ps().await?;
            // Simple table-like output
            println!("{:<20} {:<20} {:<20} {:<20}", "NAME", "IMAGE", "STATUS", "ID");
            for c in results {
                if services.is_empty() || services.contains(&c.name) {
                    println!("{:<20} {:<20} {:<20} {:<20}", c.name, c.image, c.status, c.id);
                }
            }
        }
        Commands::Logs { follow: _, tail, timestamps: _, services } => {
            let logs = engine.logs(services.first().map(|s| s.as_str()), tail).await?;
            print!("{}", logs.stdout);
            eprint!("{}", logs.stderr);
        }
        Commands::Exec { service, cmd, env: _, workdir: _, user: _ } => {
            let logs = engine.exec(&service, &cmd).await?;
            print!("{}", logs.stdout);
            eprint!("{}", logs.stderr);
        }
        Commands::Config { format, resolve_image_digests: _ } => {
            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&engine.spec)?);
            } else {
                println!("{}", serde_yaml::to_string(&engine.spec)?);
            }
        }
        Commands::Start { services } => engine.start(&services).await?,
        Commands::Stop { services } => engine.stop(&services).await?,
        Commands::Restart { services } => engine.restart(&services).await?,
    }

    Ok(())
}
