use crate::error::Result;
use crate::backend::ContainerBackend;
use crate::service::Service;

pub async fn orchestrate_service(service_name: &str, service_entity: &Service, backend: &dyn ContainerBackend) -> Result<()> {
    if service_entity.is_running(service_name, backend).await? {
        tracing::info!(service = %service_name, "already running, skipping");
        return Ok(());
    }

    if service_entity.exists(service_name, backend).await? {
        tracing::info!(service = %service_name, "exists but stopped, starting");
        service_entity.start_command(service_name, backend).await?;
    } else {
        if service_entity.needs_build() {
            tracing::info!(service = %service_name, "building image");
            service_entity.build_command(service_name, backend).await?;
        }
        tracing::info!(service = %service_name, "creating and running");
        service_entity.run_command(service_name, backend).await?;
    }
    Ok(())
}
