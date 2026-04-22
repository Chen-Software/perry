use crate::container::types::*;
use crate::container::backend::ContainerBackend;
use crate::container::verification;
use crate::container::mod_utils::{get_global_backend_instance, backend_err_to_js};
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
) -> Result<ContainerLogs, ComposeError> {
    // 1. Verify image signature before running
    let digest = verification::verify_image(image).await
        .map_err(|e| ComposeError::VerificationFailed {
            image: image.to_string(),
            reason: e.to_string()
        })?;

    // 2. Build ephemeral ContainerSpec with security constraints
    let spec = ContainerSpec {
        image: format!("{}@{}", image, digest),
        name: Some(format!("alloy-cap-{}-{}", name, rand::random::<u32>())),
        volumes: None,
        network: if grants.network { None } else { Some("none".to_string()) },
        rm: Some(true),
        env: grants.env.clone(),
        cmd: Some(cmd.iter().map(|s| s.to_string()).collect()),
        read_only: Some(true),
        ..Default::default()
    };

    // 3. Get backend and run
    let backend = crate::container::get_global_backend_instance().await
        .map_err(|msg| ComposeError::BackendNotAvailable { name: "default".into(), reason: msg })?;

    let handle = backend.run(&spec).await?;

    // 4. Wait for completion and collect logs
    // Simplified: in a real implementation we might need to wait for exit
    let logs = backend.logs(&handle.id, None).await?;

    Ok(logs)
}
