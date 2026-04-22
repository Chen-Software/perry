use crate::backend::ContainerBackend;
use crate::error::{ComposeError, Result};
use crate::types::{ContainerHandle, ContainerInfo, ContainerLogs, ContainerSpec};

/// Orchestrate a single service: ensure it is running.
pub async fn orchestrate_service(
    id: &str,
    spec: &ContainerSpec,
    backend: &dyn ContainerBackend,
) -> Result<ContainerHandle> {
    // Check if container exists
    let info_res = backend.inspect(id).await;

    match info_res {
        Ok(info) if info.status == "running" => {
            // Already running
            Ok(ContainerHandle { id: info.id, name: Some(info.name) })
        }
        Ok(info) => {
            // Exists but not running
            backend.start(&info.id).await?;
            Ok(ContainerHandle { id: info.id, name: Some(info.name) })
        }
        Err(_) => {
            // Does not exist, run it
            backend.run(spec).await
        }
    }
}
