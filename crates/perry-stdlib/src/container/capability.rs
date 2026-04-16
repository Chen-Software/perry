//! OCI-isolated shell capability.

use super::backend::ContainerBackend;
use super::types::{ContainerError, ContainerLogs, ContainerSpec};
use super::verification;
use std::collections::HashMap;
use std::sync::Arc;

/// Configuration for the capability sandbox.
#[derive(Debug, Clone)]
pub struct CapabilityConfig {
    pub image: Option<String>,
    pub network: bool,
    pub memory_limit: Option<u64>,
    pub cpu_limit: Option<u64>,
    pub pid_limit: Option<u32>,
    pub workdir: Option<String>,
    pub env: Option<HashMap<String, String>>,
    pub verify_image: bool,
    pub timeout: Option<u32>,
}

impl Default for CapabilityConfig {
    fn default() -> Self {
        Self {
            image: None,
            network: false,
            memory_limit: Some(256 * 1024 * 1024),
            cpu_limit: Some(100_000_000),
            pid_limit: Some(64),
            workdir: Some("/work".to_string()),
            env: None,
            verify_image: true,
            timeout: Some(30),
        }
    }
}

/// Result of a capability execution.
#[derive(Debug, Clone)]
pub struct CapabilityResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Run a shell command in an OCI-isolated sandbox.
pub async fn run_capability(
    backend: &Arc<dyn ContainerBackend + Send + Sync>,
    command: &str,
    config: &CapabilityConfig,
) -> Result<CapabilityResult, ContainerError> {
    // 1. Resolve image
    let image_ref = config
        .image
        .clone()
        .unwrap_or_else(verification::get_default_base_image);

    // 2. Image verification BEFORE running
    let digest = if config.verify_image {
        verification::verify_image(&image_ref).await?
    } else {
        String::new()
    };

    let image = if digest.is_empty() { image_ref } else { format!("{}@{}", image_ref, digest) };

    // 3. Build container spec
    let container_name = format!(
        "perry-cap-{:08x}",
        rand::random::<u32>()
    );

    let mut env = config.env.clone().unwrap_or_default();
    env.insert("PERRY_CAPABILITY".to_string(), "1".to_string());

    let spec = perry_container_compose::types::ContainerSpec {
        image,
        name: Some(container_name),
        ports: None,
        volumes: Some(vec![]),
        env: Some(env),
        cmd: Some(vec!["/bin/sh".to_string(), "-c".to_string(), command.to_string()]),
        entrypoint: None,
        network: if config.network {
            None
        } else {
            Some("none".to_string())
        },
        rm: Some(true),
    };

    // 5. Run the container
    let handle = backend.run(&spec).await.map_err(map_compose_err)?;

    // 6. Wait for completion
    let result = wait_for_container(backend, &handle.id, config.timeout).await;

    // 7. Get logs
    let logs = backend.logs(&handle.id, None).await.unwrap_or(perry_container_compose::types::ContainerLogs {
        stdout: String::new(),
        stderr: String::new(),
        exit_code: 0,
    });

    let exit_code = match result {
        Ok(code) => code,
        Err(_) => logs.exit_code,
    };

    Ok(CapabilityResult {
        stdout: logs.stdout,
        stderr: logs.stderr,
        exit_code,
    })
}

/// Wait for a container to finish, polling inspect every 500ms.
async fn wait_for_container(
    backend: &Arc<dyn ContainerBackend + Send + Sync>,
    id: &str,
    timeout_secs: Option<u32>,
) -> Result<i32, ContainerError> {
    let timeout = timeout_secs.unwrap_or(30);
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(timeout as u64);

    loop {
        match backend.inspect(id).await {
            Ok(info) => {
                let status = info.status.to_lowercase();
                if status.contains("exited") || status.contains("dead") {
                    return Ok(0);
                }
            }
            Err(_) => {
                return Ok(0);
            }
        }

        if tokio::time::Instant::now() >= deadline {
            return Err(ContainerError::BackendError {
                code: -1,
                message: format!("Container {} timed out after {}s", id, timeout),
            });
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
}

fn map_compose_err(e: perry_container_compose::error::ComposeError) -> ContainerError {
    match e {
        perry_container_compose::error::ComposeError::NotFound(id) => {
            ContainerError::NotFound(id)
        }
        perry_container_compose::error::ComposeError::DependencyCycle { services } => {
            ContainerError::DependencyCycle { cycle: services }
        }
        perry_container_compose::error::ComposeError::ServiceStartupFailed { service, message } => {
            ContainerError::ServiceStartupFailed { service, error: message }
        }
        perry_container_compose::error::ComposeError::ValidationError { message } => {
            ContainerError::InvalidConfig(message)
        }
        other => ContainerError::BackendError {
            code: -1,
            message: other.to_string(),
        },
    }
}
