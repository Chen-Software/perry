use super::types::{ContainerSpec, ContainerLogs, ComposeError};
use super::verification;
use super::get_global_backend_instance;

pub struct CapabilityGrants {
    pub network: bool,
    pub env: Option<std::collections::HashMap<String, String>>,
}

pub async fn alloy_container_run_capability(
    name: &str,
    image: &str,
    cmd: &[&str],
    grants: &CapabilityGrants,
) -> Result<ContainerLogs, ComposeError> {
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
        rm: Some(true),  // Always remove on exit
        env: grants.env.clone(),
        cmd: Some(cmd.iter().map(|s| s.to_string()).collect()),
        read_only: Some(true), // Read-only root filesystem
        ..Default::default()
    };

    // 3. Run via global backend instance
    let backend = get_global_backend_instance().await
        .map_err(|e| ComposeError::BackendNotAvailable { name: "global".into(), reason: e })?;

    let handle = backend.run(&spec).await?;

    // 4. Wait for completion
    backend.wait(&handle.id).await?;

    // 5. Collect output
    backend.logs(&handle.id, None).await
}
