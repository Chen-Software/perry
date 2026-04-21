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
    let _digest = verification::verify_image(image).await?;
    let spec = ContainerSpec {
        image: image.to_string(), // In real implementation use digest
        name: Some(format!("alloy-cap-{}-{}", name, rand::random::<u32>())),
        volumes: None,
        network: if grants.network { None } else { Some("none".to_string()) },
        rm: Some(true),
        env: grants.env.clone(),
        cmd: Some(cmd.iter().map(|s| s.to_string()).collect()),
        read_only: Some(true),
        ..Default::default()
    };
    let backend = get_global_backend_instance().await.map_err(|e| ComposeError::BackendNotAvailable { name: "global".into(), reason: e.to_string() })?;
    backend.run(&spec).await?;
    backend.logs(spec.name.as_ref().unwrap(), None).await
}
