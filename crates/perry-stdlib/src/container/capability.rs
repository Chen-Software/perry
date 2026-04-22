//! OCI isolation for Shell capabilities.

use std::collections::HashMap;
use crate::container::types::{ContainerLogs, ContainerError};
use crate::container::verification;
use crate::container::get_global_backend;
use crate::container::backend::{SecurityProfile, NetworkConfig, VolumeConfig};

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
    // 1. Verify image
    let _digest = verification::verify_image(image).await?;

    // 2. Build spec
    let spec = perry_container_compose::types::ContainerSpec {
        image: image.to_string(),
        name: Some(format!("alloy-cap-{}-{}", name, rand::random::<u32>())),
        network: if grants.network { None } else { Some("none".to_string()) },
        rm: Some(true),
        env: grants.env.clone(),
        cmd: Some(cmd.iter().map(|s| s.to_string()).collect()),
        ..Default::default()
    };

    let profile = SecurityProfile {
        read_only_rootfs: true,
        seccomp_profile: None,
        cap_drop: vec!["ALL".to_string()],
    };

    // 3. Run
    let backend = get_global_backend().await?;
    let handle = backend.run_with_security(&spec, &profile).await?;

    // 4. Wait and collect logs
    let logs = backend.wait_and_logs(&handle.id).await?;

    Ok(ContainerLogs {
        stdout: logs.stdout,
        stderr: logs.stderr,
    })
}
