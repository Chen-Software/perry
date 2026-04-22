use super::types::{ContainerSpec, ContainerLogs, ComposeError, ContainerHandle};
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
) -> Result<ContainerLogs, ComposeError> {
    // 1. Verify image signature before running
    let digest = verification::verify_image(image).await
        .map_err(|e| ComposeError::VerificationFailed { image: image.into(), reason: e.to_string() })?;

    // 2. Build ephemeral ContainerSpec with security constraints
    let spec = ContainerSpec {
        image: format!("{}@{}", image, digest),
        name: Some(format!("alloy-cap-{}-{}", name, rand::random::<u32>())),
        // No persistent volumes
        volumes: None,
        // No network access by default (unless grants.network == true)
        network: if grants.network { None } else { Some("none".to_string()) },
        // Read-only root filesystem
        rm: Some(true), // Always remove on exit
        env: grants.env.clone(),
        cmd: Some(cmd.iter().map(|s| s.to_string()).collect()),
        read_only: Some(true),
        ..Default::default()
    };

    // 3. Run via global backend
    let backend = get_global_backend_instance().await
        .map_err(|e| ComposeError::BackendNotAvailable { name: "global".into(), reason: e })?;

    let handle = backend.run(&spec).await?;

    // 4. Wait for completion
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500));
    loop {
        interval.tick().await;
        match backend.inspect(&handle.id).await {
            Ok(info) => {
                if info.status == "exited" || info.status == "stopped" {
                    break;
                }
            }
            Err(_) => break, // Container might have been auto-removed
        }
    }

    // 5. Collect output
    let logs = backend.logs(&handle.id, None).await?;

    Ok(logs)
}
