use std::collections::HashMap;
use crate::container::error::ContainerError;
use crate::container::types::{ContainerLogs, ContainerSpec};
use crate::container::backend::ContainerBackend;
use std::sync::Arc;

pub struct CapabilityGrants {
    pub network: bool,
    pub env: Option<HashMap<String, String>>,
}

pub async fn alloy_container_run_capability(
    _name: &str,
    image: &str,
    cmd: &[&str],
    grants: &CapabilityGrants,
    backend: Arc<dyn ContainerBackend>
) -> Result<ContainerLogs, ContainerError> {
    crate::container::verification::verify_image(image)?;

    let spec = ContainerSpec {
        image: image.to_string(),
        cmd: Some(cmd.iter().map(|s| s.to_string()).collect()),
        env: grants.env.clone(),
        network: if grants.network { None } else { Some("none".to_string()) },
        rm: Some(true),
        ..Default::default()
    };

    backend.run(&spec).await.map(|id| {
        // Collect logs for ephemeral container
        ContainerLogs { stdout: id, stderr: String::new() }
    }).map_err(|e| ContainerError::BackendError { code: 1, message: e.to_string() })
}
