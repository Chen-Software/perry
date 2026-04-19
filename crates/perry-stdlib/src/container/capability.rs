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
        ..Default::default()
    };

    let backend = get_global_backend_instance().await?;
    let crate_spec = perry_container_compose::types::ContainerSpec {
        image: spec.image.clone(),
        name: spec.name.clone(),
        ports: spec.ports.clone(),
        volumes: spec.volumes.clone(),
        env: spec.env.clone(),
        cmd: spec.cmd.clone(),
        entrypoint: spec.entrypoint.clone(),
        network: spec.network.clone(),
        rm: spec.rm.clone(),
    };

    let handle = backend.run(&crate_spec).await.map_err(|e| ContainerError::BackendError { code: -1, message: e.to_string() })?;

    let logs = backend.logs(&handle.id, None).await.map_err(|e| ContainerError::BackendError { code: -1, message: e.to_string() })?;
    Ok(ContainerLogs::from(logs))
}
