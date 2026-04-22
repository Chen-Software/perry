use crate::container::types::{ContainerError, ContainerLogs, ContainerSpec};
use crate::container::verification::verify_image;
use crate::container::ContainerBackend;
use std::sync::Arc;

pub async fn alloy_container_run_capability(
    _name: &str,
    image: &str,
    cmd: &[String],
    backend: Arc<dyn ContainerBackend>,
) -> Result<ContainerLogs, ContainerError> {
    // 1. Verify image signature
    verify_image(image).await?;

    // 2. Configure ephemeral container
    let spec = ContainerSpec {
        image: image.to_string(),
        name: None,
        ports: None,
        volumes: None,
        env: None,
        cmd: Some(cmd.to_vec()),
        entrypoint: None,
        network: Some("none".into()),
        rm: Some(true),
    };

    // 3. Run and capture logs
    let handle = backend.run(&spec).await.map_err(ContainerError::from)?;
    let logs = backend.logs(&handle.id, None).await.map_err(ContainerError::from)?;

    // 4. Remove
    let _ = backend.remove(&handle.id, true).await;

    Ok(logs)
}
