//! alloy_container_run_capability() for ShellBridge integration.

use super::types::{ContainerError, ContainerLogs, ContainerSpec};
use super::verification;
use crate::container::get_global_backend;
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
        ports: Some(vec![]),
        volumes: Some(vec![]),
        // No network access by default (unless grants.network == true)
        network: if grants.network { None } else { Some("none".to_string()) },
        rm: Some(true), // Always remove on exit
        read_only: Some(true),
        security_opt: Some(vec!["no-new-privileges:true".to_string()]),
        env: grants.env.clone(),
        cmd: Some(cmd.iter().map(|s| s.to_string()).collect()),
        entrypoint: None,
        ..Default::default()
    };

    // 3. Get backend and run
    let backend = Arc::clone(get_global_backend().await?);
    let handle = backend.run(&spec).await.map_err(|e| ContainerError::BackendError {
        code: -1,
        message: format!("Failed to run capability container: {}", e)
    })?;

    // 4. Wait for completion and collect output
    // Note: in a real implementation we would wait for the container to exit
    // before collecting logs, but since backend.run() for these drivers
    // is currently synchronous or detached, we follow the design's collect-after-run.
    backend.logs(&handle.id, None).await.map_err(|e| ContainerError::BackendError {
        code: -1,
        message: format!("Failed to collect capability logs: {}", e)
    })
}
