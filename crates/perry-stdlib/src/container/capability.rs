//! OCI isolation for sandboxed shellCapabilities

use crate::container::types::{ComposeError, Result};
use crate::container::types::{ContainerSpec, ContainerLogs};
use crate::container::verification::verify_image;
use crate::container::get_global_backend_instance;
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
) -> Result<ContainerLogs> {
    // 1. Image verification
    verify_image(image).await?;

    // 2. Build ContainerSpec with strict security profile
    let spec = ContainerSpec {
        image: image.to_string(),
        name: Some(format!("perry-cap-{}", name)),
        cmd: Some(cmd.iter().map(|s| s.to_string()).collect()),
        network: if grants.network { None } else { Some("none".into()) },
        rm: Some(true),
        read_only: Some(true),
        env: grants.env.clone(),
        ..Default::default()
    };

    // 3. Run ephemeral container
    let backend = get_global_backend_instance().await.map_err(|e| ComposeError::BackendNotAvailable { name: "global".into(), reason: e })?;
    let handle = backend.run(&spec).await?;

    // 4. Collect logs and wait
    let logs = backend.logs(&handle.id, None).await?;
    let _ = backend.wait(&handle.id).await?;

    Ok(logs)
}
