use crate::backend::ContainerBackend;
use crate::error::Result;
use crate::service::Service;

pub async fn orchestrate_service(
    service: &Service,
    service_name: &str,
    backend: &dyn ContainerBackend,
) -> Result<()> {
    if service.is_running(service_name, backend).await? {
        tracing::info!(service = %service_name, "already running, skipping");
        return Ok(());
    }

    if service.exists(service_name, backend).await? {
        tracing::info!(service = %service_name, "exists but stopped, starting");
        service.start_command(service_name, backend).await?;
    } else {
        if service.needs_build(backend).await {
            tracing::info!(service = %service_name, "building image");
            service.build_command(service_name, backend).await?;
        }
        tracing::info!(service = %service_name, "creating and running");
        service.run_command(service_name, backend).await?;
    }
    Ok(())
}
