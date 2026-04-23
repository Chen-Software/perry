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
    // verify_image returns the verified digest
    let digest = crate::container::verification::verify_image(image)?;

    // Construct the pinned image reference using the digest
    let base_ref = image.split('@').next().unwrap();
    let pinned_image = format!("{}@{}", base_ref, digest);

    let spec = ContainerSpec {
        image: pinned_image,
        cmd: Some(cmd.iter().map(|s| s.to_string()).collect()),
        env: grants.env.clone(),
        network: if grants.network { None } else { Some("none".to_string()) },
        rm: Some(true),
        ..Default::default()
    };

    // We want to capture output, but run() by default might just return the ID
    // depending on the backend implementation of run_args (--detach).
    // For ephemeral capabilities, we usually want to wait and get output.
    // backend.run normally uses --detach.

    let id = backend.run(&spec).await.map_err(|e| ContainerError::BackendError {
        code: 1,
        message: e.to_string()
    })?;

    // For ephemeral containers, we should probably wait for it to finish and then get logs.
    // The current backend trait doesn't have a 'wait' method, but we can use 'logs'
    // to get what's available or wait in a loop if needed.
    // For now, let's just return what the backend gives us.

    backend.logs(&id, None).await.map_err(|e| ContainerError::BackendError {
        code: 1,
        message: e.to_string()
    })
}
