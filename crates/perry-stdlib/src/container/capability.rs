use std::collections::HashMap;
use crate::container::types::{ContainerSpec, ContainerLogs, ContainerError};
use super::get_global_backend;
use super::verification;

pub struct CapabilityGrants {
    pub network: bool,
    pub env: Option<HashMap<String, String>>,
}

pub async fn alloy_container_run_capability(name: &str, image: &str, cmd: &[&str], grants: &CapabilityGrants) -> Result<ContainerLogs, ContainerError> {
    let _digest = verification::verify_image(image).await?;
    let backend = get_global_backend().await?;
    let spec = ContainerSpec {
        image: image.to_string(),
        name: Some(format!("alloy-cap-{}-{}", name, rand::random::<u32>())),
        network: if grants.network { None } else { Some("none".to_string()) },
        rm: Some(true),
        env: grants.env.clone(),
        cmd: Some(cmd.iter().map(|s| s.to_string()).collect()),
        ..Default::default()
    };
    let h = backend.run(&spec).await.map_err(ContainerError::from)?;
    backend.wait_and_logs(&h.id).await.map_err(ContainerError::from)
}
