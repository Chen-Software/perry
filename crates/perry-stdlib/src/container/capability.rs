use crate::container::types::{ContainerError, ContainerLogs, ContainerSpec};
use crate::container::verification;
use crate::container::backend::get_global_backend;
use std::sync::Arc;

pub struct CapabilityGrants {
    pub network: bool,
    pub env: Option<std::collections::HashMap<String, String>>,
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
        volumes: None,
        // No network access by default (unless grants.network == true)
        network: if grants.network { None } else { Some("none".to_string()) },
        rm: Some(true), // Always remove on exit
        env: grants.env.clone(),
        cmd: Some(cmd.iter().map(|s| s.to_string()).collect()),
        ..Default::default()
    };

    // 3. Run with security profile (read-only rootfs)
    let backend = get_global_backend().await?;
    let profile = perry_container_compose::backend::SecurityProfile {
        read_only_rootfs: true,
        seccomp_profile: None,
    };
    let handle = backend.run_with_security(&spec, &profile).await
        .map_err(|e| ContainerError::BackendError { code: 1, message: e.to_string() })?;

    // 4. Wait for completion and collect output
    let _exit_code = backend.wait(&handle.id).await
        .map_err(|e| ContainerError::BackendError { code: 1, message: e.to_string() })?;

    let logs = backend.logs(&handle.id, None).await
        .map_err(|e| ContainerError::BackendError { code: 1, message: e.to_string() })?;

    // 5. Container is auto-removed (rm: true)
    Ok(logs)
}
