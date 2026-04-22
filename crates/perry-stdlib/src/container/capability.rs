use std::collections::HashMap;
use crate::types::{ComposeError, ContainerLogs, ContainerSpec};
use crate::container::verification;

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
    let digest = verification::verify_image(image).await?;

    let spec = ContainerSpec {
        image: format!("{}@{}", image, digest),
        name: Some(format!("alloy-cap-{}-{}", name, rand::random::<u32>())),
        volumes: None,
        network: if grants.network { None } else { Some("none".to_string()) },
        rm: Some(true),
        env: grants.env.clone(),
        cmd: Some(cmd.iter().map(|s| s.to_string()).collect()),
        entrypoint: None,
        ports: None,
    };

    let backend = crate::mod_impl::get_global_backend_instance_internal().await
        .map_err(|e| ComposeError::BackendNotAvailable { name: "default".into(), reason: e })?;

    let handle = backend.run(&spec).await?;
    let logs = backend.logs(&handle.id, None).await?;

    Ok(logs)
}
