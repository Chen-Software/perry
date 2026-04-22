use crate::container::types::{ContainerLogs, ContainerSpec};
use crate::container::{verification, get_global_backend_instance};
use perry_container_compose::error::ComposeError as ContainerError;
use std::collections::HashMap;

pub struct CapabilityGrants {
    pub network: bool,
    pub env: Option<HashMap<String, String>>,
}

/// Run a shell capability in an ephemeral, sandboxed container.
pub async fn alloy_container_run_capability(
    name: &str,
    image: &str,
    cmd: &[&str],
    grants: &CapabilityGrants,
) -> Result<ContainerLogs, ContainerError> {
    // 1. Verify image signature before running (Requirement 13.1, 15.1)
    let digest = verification::verify_image(image).await?;

    // 2. Build ephemeral ContainerSpec with security constraints (Requirement 13.3)
    let spec = ContainerSpec {
        image: format!("{}@{}", image, digest),
        name: Some(format!("alloy-cap-{}-{}", name, rand::random::<u32>())),
        volumes: None,
        network: if grants.network { None } else { Some("none".to_string()) },
        rm: Some(true),
        read_only: Some(true),
        seccomp: Some("default".into()), // Requirement 13.3
        env: grants.env.clone(),
        cmd: Some(cmd.iter().map(|s| s.to_string()).collect()),
        ..Default::default()
    };

    // 3. Run via backend (Requirement 13.2)
    let backend = get_global_backend_instance().map_err(|e| ContainerError::ValidationError { message: e.to_string() })?;
    let handle = backend.run(&spec).await?;

    // 4. Collect output
    let logs = backend.logs(&handle.id, None).await?;

    // 5. Container is auto-removed by rm: true
    Ok(logs)
}
