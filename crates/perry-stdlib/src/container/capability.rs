//! alloy_container_run_capability() for ShellBridge integration.

use super::types::{ContainerError, ContainerLogs, ContainerSpec};
use super::verification;
use super::mod_priv::get_global_backend_instance;
use std::collections::HashMap;

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
        // Always remove on exit
        rm: Some(true),
        env: grants.env.clone(),
        cmd: Some(cmd.iter().map(|s| s.to_string()).collect()),
        entrypoint: None,
        ..Default::default()
    };

    // 3. Get backend and run
    let backend = get_global_backend_instance().await.map_err(|e| ContainerError::BackendError { code: -1, message: e })?;
    let handle = backend.run(&spec).await.map_err(|e| ContainerError::BackendError { code: -1, message: e.to_string() })?;

    // 4. Collect logs (container is auto-removed by rm: true when it exits)
    backend.logs(&handle.id, None).await.map_err(|e| ContainerError::BackendError { code: -1, message: e.to_string() })
}
