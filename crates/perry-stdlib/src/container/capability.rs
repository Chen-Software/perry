//! OCI isolation for Shell capabilities.

use std::collections::HashMap;
use crate::container::types::{ContainerSpec, ContainerLogs};
use crate::container::verification;
use crate::container::get_global_backend_instance;

pub struct CapabilityGrants {
    pub network: bool,
    pub env: Option<HashMap<String, String>>,
}

pub async fn alloy_container_run_capability(
    name: &str,
    image: &str,
    cmd: &[&str],
    grants: &CapabilityGrants,
) -> Result<ContainerLogs, crate::container::types::ContainerError> {
    let digest = verification::verify_image(image).await?;
    let spec = ContainerSpec {
        image: format!("{}@{}", image, digest),
        name: Some(format!("alloy-cap-{}-{}", name, rand::random::<u32>())),
        network: if grants.network { None } else { Some("none".to_string()) },
        rm: Some(true),
        read_only: Some(true),
        env: grants.env.clone(),
        cmd: Some(cmd.iter().map(|s| s.to_string()).collect()),
        ..Default::default()
    };

    let backend = get_global_backend_instance().await?;
    let handle = backend.run(&spec).await.map_err(crate::container::types::ContainerError::from)?;
    backend.wait(&handle.id).await.map_err(crate::container::types::ContainerError::from)?;
    let logs = backend.logs(&handle.id, None, false).await.map_err(crate::container::types::ContainerError::from)?;

    Ok(ContainerLogs { stdout: logs.stdout, stderr: logs.stderr })
}
