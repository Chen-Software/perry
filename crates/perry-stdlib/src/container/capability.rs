//! alloy_container_run_capability() for ShellBridge integration.

use super::types::{ContainerError, ContainerLogs, ContainerSpec};
use super::verification;
use crate::container::mod_priv::get_global_backend_instance;
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
    let digest = verification::verify_image(image).await?;

    let spec = ContainerSpec {
        image: format!("{}@{}", image, digest),
        name: Some(format!("alloy-cap-{}-{}", name, rand::random::<u32>())),
        ports: Some(vec![]),
        volumes: Some(vec![]),
        network: if grants.network { None } else { Some("none".to_string()) },
        rm: Some(true),
        env: grants.env.clone(),
        cmd: Some(cmd.iter().map(|s| s.to_string()).collect()),
        entrypoint: None,
    };

    let backend = get_global_backend_instance();
    let handle = backend.run(&spec).await.map_err(|e| ContainerError::BackendError { code: -1, message: e.to_string() })?;

    let logs = backend.logs(&handle.id, None).await.map_err(|e| ContainerError::BackendError { code: -1, message: e.to_string() })?;

    // Ensure cleanup
    let _ = backend.stop(&handle.id, Some(0)).await;
    let _ = backend.remove(&handle.id, true).await;

    Ok(logs)
}
