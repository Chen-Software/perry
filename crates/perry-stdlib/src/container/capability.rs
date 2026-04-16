//! alloy_container_run_capability() for ShellBridge integration.

use super::types::{ContainerError, ContainerLogs, ContainerSpec};
use super::verification;
use super::get_global_backend;
use std::collections::HashMap;
use std::sync::Arc;

pub struct CapabilityGrants {
    pub network: bool,
    pub env: Option<HashMap<String, String>>,
}

pub async fn alloy_container_run_capability(
    name: &str,
    image: &str,
    cmd: &[&str],
    grants: &CapabilityGrants,
) -> Result<ContainerLogs, ContainerError> {
    // 1. Verify image signature before running
    let digest = verification::verify_image(image).await?;

    // 2. Build ephemeral ContainerSpec with security constraints
    let spec = ContainerSpec {
        image: format!("{}@{}", image, digest),
        name: Some(format!("alloy-cap-{}-{}", name, rand::random::<u32>())),
        // No persistent volumes
        volumes: Some(vec![]),
        // No network access by default (unless grants.network == true)
        network: if grants.network { None } else { Some("none".to_string()) },
        // Read-only root filesystem
        read_only: Some(true),
        rm: Some(true), // Always remove on exit
        env: grants.env.clone(),
        cmd: Some(cmd.iter().map(|s| s.to_string()).collect()),
        entrypoint: None,
        ..Default::default()
    };

    // 3. Run with security constraints
    let backend = Arc::clone(get_global_backend().await?);
    let handle = backend.run(&spec).await.map_err(|e| ContainerError::BackendError {
        code: -1,
        message: format!("Failed to run sandboxed container for capability {}: {}", name, e)
    })?;

    // 4. Collect output and return
    backend.logs(&handle.id, None).await.map_err(|e| ContainerError::BackendError {
        code: -1,
        message: format!("Failed to collect logs for capability {}: {}", name, e)
    })
}
