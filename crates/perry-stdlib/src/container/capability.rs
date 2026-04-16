//! OCI Isolation for Shell Capabilities.

use crate::container::backend::ContainerBackend;
use crate::container::types::ContainerLogs;
use std::collections::HashMap;
use std::sync::Arc;

pub struct CapabilityConfig {
    pub network: bool,
    pub env: HashMap<String, String>,
}

impl Default for CapabilityConfig {
    fn default() -> Self {
        Self {
            network: false,
            env: HashMap::new(),
        }
    }
}

pub async fn run_capability(
    backend: &Arc<dyn ContainerBackend>,
    command: &str,
    config: &CapabilityConfig,
) -> Result<ContainerLogs, perry_container_compose::error::ComposeError> {
    // 1. Image verification
    let image = "alpine";
    super::verification::verify_image(image).await?;

    // 2. Build ContainerSpec
    let spec = crate::container::types::ContainerSpec {
        image: image.to_string(),
        cmd: Some(vec!["sh".into(), "-c".into(), command.into()]),
        network: if config.network { None } else { Some("none".into()) },
        rm: Some(true),
        ..Default::default()
    };

    backend.run(&spec).await?;

    // In a real impl, we'd wait for completion and collect logs
    Ok(ContainerLogs {
        stdout: "Capability executed".into(),
        stderr: "".into(),
    })
}
