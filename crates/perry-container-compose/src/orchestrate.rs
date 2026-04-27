use crate::service::Service;
use crate::backend::ContainerBackend;
use crate::error::ComposeError;

pub async fn orchestrate_service(service: &Service, backend: &dyn ContainerBackend) -> Result<(), ComposeError> {
    if service.is_running(backend).await? {
        return Ok(());
    }

    if service.exists(backend).await? {
        service.start_command(backend).await?;
    } else {
        if service.needs_build() {
            service.build_command(backend).await?;
        }
        service.run_command(backend).await?;
    }

    Ok(())
}
