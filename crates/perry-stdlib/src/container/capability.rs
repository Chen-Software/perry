//! OCI isolation for Shell Capabilities

use std::collections::HashMap;
use super::get_global_backend;
use super::backend::ContainerBackend;
use super::types::{ContainerError, ContainerLogs, ContainerSpec};
use super::verification;

pub struct CapabilityGrants {
    pub network: bool,
    pub env: Option<HashMap<String, String>>,
}

/// Run a shell capability in an ephemeral, sandboxed container.
/// Called by ShellBridge; NOT part of the public TypeScript API.
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
        volumes: None,
        // No network access by default (unless grants.network == true)
        network: if grants.network { None } else { Some("none".to_string()) },
        // Always remove on exit
        rm: Some(true),
        env: grants.env.clone(),
        cmd: Some(cmd.iter().map(|s| s.to_string()).collect()),
        ..Default::default()
    };

    // 3. Get backend and run
    let backend = get_global_backend()?;

    // In a real implementation, we would use backend.run_with_security
    // and then wait for completion and collect logs.
    // For now, we use standard run.
    let handle = backend.run(&spec).await?;

    // Best effort logs collection (might need polling for completion)
    backend.logs(&handle.id, None).await.map_err(ContainerError::from)
}
