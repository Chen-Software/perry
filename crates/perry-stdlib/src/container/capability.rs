use std::collections::HashMap;
use std::sync::Arc;
use crate::container::types::{ContainerLogs, ContainerSpec};
use crate::container::verification;
use crate::container::backend::ContainerBackend;
use crate::container::ComposeError;

pub struct CapabilityGrants {
    pub network: bool,
    pub env: Option<HashMap<String, String>>,
}

pub async fn alloy_container_run_capability(
    name: &str,
    image: &str,
    cmd: &[&str],
    grants: &CapabilityGrants,
    backend: &Arc<dyn ContainerBackend>,
) -> Result<ContainerLogs, ComposeError> {
    let digest = verification::verify_image(image).await?;

    let spec = ContainerSpec {
        image: format!("{}@{}", image, digest),
        name: Some(format!("alloy-cap-{}-{}", name, rand::random::<u32>())),
        network: if grants.network { None } else { Some("none".to_string()) },
        env: grants.env.clone(),
        cmd: Some(cmd.iter().map(|s| s.to_string()).collect()),
        rm: Some(true),
        read_only: Some(true),
        ..Default::default()
    };

    let handle = backend.run(&spec).await?;
    let logs = backend.logs(&handle.id, None).await?;
    let _ = backend.remove(&handle.id, true).await;
    Ok(logs)
}
