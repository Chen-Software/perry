use crate::error::Result;
use crate::types::ComposeService;
use crate::backend::ContainerBackend;
use crate::commands::{BuildCommand, RunCommand, StartCommand, ContainerCommand};

pub async fn orchestrate_service(
    service_name: &str,
    service: &ComposeService,
    backend: &dyn ContainerBackend,
) -> Result<()> {
    let generated_name = crate::service::generate_name(service_name, service)?;

    // Check if container already exists
    let existing = backend.list(true).await?;
    let found = existing.iter().find(|c| c.name == generated_name || c.id == generated_name);

    if let Some(info) = found {
        if info.status.to_lowercase().contains("running") || info.status.to_lowercase().contains("up") {
            tracing::info!(service = service_name, "already running, skipping");
            return Ok(());
        }
        tracing::info!(service = service_name, "exists but stopped, starting");
        StartCommand { container_id: info.id.clone() }.exec(backend).await?;
    } else {
        if crate::service::needs_build(service) {
            tracing::info!(service = service_name, "building image");
            BuildCommand { service_name, service }.exec(backend).await?;
        }
        tracing::info!(service = service_name, "creating and running");
        RunCommand { service_name, service }.exec(backend).await?;
    }
    Ok(())
}
