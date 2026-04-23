use crate::error::Result;
use crate::backend::ContainerBackend;
use crate::service::Service;

pub async fn orchestrate_service(service: &Service, backend: &dyn ContainerBackend) -> Result<()> {
    if service.is_running(backend).await? {
        tracing::info!(service = %service.name(), "already running, skipping");
        return Ok(());
    }

    if service.exists(backend).await? {
        tracing::info!(service = %service.name(), "exists but stopped, starting");
        service.start_command(backend).await?;
    } else {
        if service.needs_build() {
            tracing::info!(service = %service.name(), "building image");
            service.build_command(backend).await?;
        }
        tracing::info!(service = %service.name(), "creating and running");
        service.run_command(backend).await?;
    }
    Ok(())
}
