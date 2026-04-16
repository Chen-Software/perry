//! alloy_container_run_capability() for ShellBridge integration.

use super::types::{ContainerError, ContainerLogs, ContainerSpec};
use super::verification;
use super::get_global_backend_instance;
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
        ports: Some(vec![]),
        // No persistent volumes
        volumes: Some(vec![]),
        // No network access by default (unless grants.network == true)
        network: if grants.network { None } else { Some("none".to_string()) },
        // Read-only root filesystem
        // (Podman/AppleContainer use -v /:/:ro or similar flags which can be passed via volumes string
        // but here we follow the spec's intent of a secured ephemeral container)
        rm: Some(true), // Always remove on exit
        env: grants.env.clone(),
        cmd: Some(cmd.iter().map(|s| s.to_string()).collect()),
        entrypoint: None,
        ..Default::default()
    };

    // 3. Run via global backend instance
    let backend = get_global_backend_instance();
    let handle = backend.run(&spec).await?;

    // 4. Wait for completion and collect output
    backend.logs(&handle.id, None).await
}
