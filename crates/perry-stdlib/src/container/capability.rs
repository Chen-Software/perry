//! Sandboxed OCI capability for isolated command execution.

use crate::container::backend::ContainerBackend;
use crate::container::verification;
use perry_container_compose::types::{ContainerLogs, ContainerSpec};
use std::collections::HashMap;
use std::sync::Arc;

pub struct CapabilityConfig {
    pub image: String,
    pub network: bool,
    pub env: HashMap<String, String>,
}

impl Default for CapabilityConfig {
    fn default() -> Self {
        CapabilityConfig {
            image: "cgr.dev/chainguard/wolfi-base:latest".into(),
            network: false,
            env: HashMap::new(),
        }
    }
}

/// Run a command in an ephemeral sandboxed container.
pub async fn run_capability(
    backend: &Arc<dyn ContainerBackend>,
    command: &str,
    config: &CapabilityConfig,
) -> Result<ContainerLogs, String> {
    // 1. Verify image
    verification::verify_image(&config.image).await?;

    // 2. Build spec
    let spec = ContainerSpec {
        image: config.image.clone(),
        name: Some(format!("perry-cap-{}", rand::random::<u32>())),
        ports: None,
        volumes: None,
        env: Some(config.env.clone()),
        cmd: Some(vec!["sh".into(), "-c".into(), command.to_string()]),
        entrypoint: None,
        network: if config.network { None } else { Some("none".to_string()) },
        rm: Some(true),
    };

    // 3. Run and wait for logs
    let handle = backend.run(&spec).await.map_err(|e| e.to_string())?;
    backend.logs(&handle.id, None).await.map_err(|e| e.to_string())
}
