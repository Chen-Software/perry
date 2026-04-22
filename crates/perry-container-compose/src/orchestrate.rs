use crate::error::ComposeError;
use crate::service::Service;
use crate::backend::ContainerBackend;

pub async fn orchestrate_service(service_name: &str, service: &Service, backend: &dyn ContainerBackend) -> Result<(), ComposeError> {
    if service.is_running(service_name, backend).await? {
        tracing::info!(service = %service_name, "already running, skipping");
        return Ok(());
    }
    if service.exists(service_name, backend).await? {
        tracing::info!(service = %service_name, "exists but stopped, starting");
        service.start_command(service_name, backend).await?;
    } else {
        if service.needs_build() {
            tracing::info!(service = %service_name, "building image");
            service.build_command(service_name, backend).await?;
        }
        tracing::info!(service = %service_name, "creating and running");
        service.run_command(service_name, backend).await?;
    }
    Ok(())
}
