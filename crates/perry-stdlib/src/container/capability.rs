use std::collections::HashMap;
use crate::container::verification;
use crate::container::types::{ContainerSpec, ContainerLogs};
use crate::container::backend::detect_backend;

pub struct CapabilityGrants {
    pub network: bool,
    pub env: Option<HashMap<String, String>>,
}

pub async fn alloy_container_run_capability(
    name: &str,
    image: &str,
    cmd: &[&str],
    grants: &CapabilityGrants,
) -> Result<ContainerLogs, String> {
    let digest = verification::verify_image(image).await?;

    let spec = ContainerSpec {
        image: format!("{}@{}", image, digest),
        name: Some(format!("perry-cap-{}-{}", name, rand::random::<u32>())),
        network: if grants.network { None } else { Some("none".to_string()) },
        rm: Some(true),
        read_only: Some(true),
        env: grants.env.clone(),
        cmd: Some(cmd.iter().map(|s| s.to_string()).collect()),
        ..Default::default()
    };

    let backend = detect_backend().await.map_err(|_| "No backend found")?;
    let handle = backend.run(&spec).await.map_err(|e| e.to_string())?;

    // Wait and get logs (simplified)
    let logs = backend.logs(&handle.id, None).await.map_err(|e| e.to_string())?;
    let _ = backend.remove(&handle.id, true).await;

    Ok(logs)
}
