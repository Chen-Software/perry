use crate::types::ComposeService;
use crate::backend::ContainerBackend;
use crate::error::Result;

pub async fn orchestrate_service(
    service_key: &str,
    service: &ComposeService,
    backend: &dyn ContainerBackend,
) -> Result<()> {
    if service.is_running(service_key, backend).await? {
        tracing::info!(service = %service_key, "already running, skipping");
        return Ok(());
    }

    if service.exists(service_key, backend).await? {
        tracing::info!(service = %service_key, "exists but stopped, starting");
        service.start_command(service_key, backend).await?;
    } else {
        tracing::info!(service = %service_key, "creating and running");
        service.run_command(service_key, backend).await?;
    }

    Ok(())
}
