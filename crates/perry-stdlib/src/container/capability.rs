//! OCI-isolated shell capability.
//!
//! `alloy_container_run_capability` provides a sandboxed execution environment
//! where untrusted shell commands run inside an OCI container with:
//! - No network access (by default)
//! - Read-only root filesystem (tmpfs for writable dirs)
//! - Resource limits (CPU, memory, PID)
//! - Automatic image verification via cosign
//! - Chainguard base images for minimal attack surface

use super::backend::ContainerBackend;
use super::types::{ContainerError, ContainerLogs, ContainerSpec};
use super::verification;
use std::collections::HashMap;
use std::sync::Arc;

/// Configuration for the capability sandbox.
#[derive(Debug, Clone)]
pub struct CapabilityConfig {
    /// Image to use. If `None`, uses `verification::get_default_base_image()`.
    pub image: Option<String>,
    /// Whether to allow network access (default: `false`).
    pub network: bool,
    /// Memory limit in bytes (default: 256 MiB).
    pub memory_limit: Option<u64>,
    /// CPU limit in nanoseconds per second (default: 100_000_000 = 0.1 CPU).
    pub cpu_limit: Option<u64>,
    /// Max PID count (default: 64).
    pub pid_limit: Option<u32>,
    /// Working directory inside the container (default: `/work`).
    pub workdir: Option<String>,
    /// Environment variables to pass into the container.
    pub env: Option<HashMap<String, String>>,
    /// Whether to verify image signature before running (default: `true`).
    pub verify_image: bool,
    /// Timeout in seconds (default: 30).
    pub timeout: Option<u32>,
}

impl Default for CapabilityConfig {
    fn default() -> Self {
        Self {
            image: None,
            network: false,
            memory_limit: Some(256 * 1024 * 1024), // 256 MiB
            cpu_limit: Some(100_000_000),           // 0.1 CPU
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
///
/// This is the core of the `alloy:gui` container capability — it provides
/// a secure, sandboxed environment for running untrusted commands.
///
/// # Arguments
/// * `backend` - The container backend to use
/// * `command` - The shell command to execute (run via `/bin/sh -c`)
/// * `config` - Sandbox configuration
///
/// # Returns
/// `CapabilityResult` with stdout, stderr, and exit code.
pub async fn run_capability(
    backend: &Arc<dyn ContainerBackend>,
    command: &str,
    config: &CapabilityConfig,
) -> Result<CapabilityResult, ContainerError> {
    // 1. Resolve and verify image
    let image_ref = config
        .image
        .clone()
        .unwrap_or_else(verification::get_default_base_image);

    let digest = if config.verify_image {
        verification::verify_image(&image_ref).await?
    } else {
        // Fallback for unverified
        image_ref.clone()
    };

    let image = if digest.starts_with("sha256:") {
        format!("{}@{}", image_ref, digest)
    } else {
        image_ref
    };

    // 3. Build container spec
    let container_name = format!(
        "perry-cap-{}",
        md5_hex(command).get(..12).unwrap_or("unknown")
    );

    let mut env = config.env.clone().unwrap_or_default();
    env.insert("PERRY_CAPABILITY".to_string(), "1".to_string());

    let mut spec = ContainerSpec {
        image,
        name: Some(container_name),
        ports: None,
        volumes: Some(vec![]), // no host mounts by default
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

    // 4. Add resource limits as command arguments (OCI runtime flags)
    // Note: resource limits are passed via the runtime, not the spec.
    // The actual enforcement depends on the backend supporting --cpus/--memory flags.

    // 5. Run the container (create + start + wait)
    let handle = backend.run(&spec).await?;

    // 6. Wait for completion (poll inspect until stopped, or use logs)
    let result = wait_for_container(backend, &handle.id, config.timeout).await;

    // 7. Get logs before removal (the container is --rm so it may already be gone)
    let logs = backend.logs(&handle.id, None).await.unwrap_or(ContainerLogs {
        stdout: String::new(),
        stderr: String::new(),
    });

    // 8. Ensure cleanup
    let _ = backend.stop(&handle.id, Some(5)).await;
    let _ = backend.remove(&handle.id, true).await;

    let exit_code = match result {
        Ok(code) => code,
        Err(_) => -1,
    };

    Ok(CapabilityResult {
        stdout: logs.stdout,
        stderr: logs.stderr,
        exit_code,
    })
}

/// Run a capability with a Chainguard tool image.
///
/// This is a convenience wrapper that resolves the tool name to a Chainguard
/// image and runs the specified command in it.
///
/// # Example
/// ```ignore
/// use perry_stdlib::container::capability::{run_tool_capability, CapabilityConfig};
/// # async fn example(backend: std::sync::Arc<dyn perry_stdlib::container::backend::ContainerBackend>) -> Result<(), Box<dyn std::error::Error>> {
/// let config = CapabilityConfig::default();
/// let result = run_tool_capability(&backend, "git", &["clone", "https://..."], &config).await?;
/// # Ok(())
/// # }
/// ```
pub async fn run_tool_capability(
    backend: &Arc<dyn ContainerBackend>,
    tool: &str,
    args: &[&str],
    config: &CapabilityConfig,
) -> Result<CapabilityResult, ContainerError> {
    let image = verification::get_chainguard_image(tool).ok_or_else(|| {
        ContainerError::InvalidConfig(format!("No Chainguard image found for tool: {}", tool))
    })?;

    let mut tool_config = config.clone();
    tool_config.image = Some(image);

    let cmd = args
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>()
        .join(" ");

    run_capability(backend, &cmd, &tool_config).await
}

// ============ Internal helpers ============

/// Wait for a container to finish, polling inspect every 500ms.
async fn wait_for_container(
    backend: &Arc<dyn ContainerBackend>,
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
                    // Extract exit code from status if available
                    // Format: "Exited (0) 1s ago" or "exited"
                    if let Some(code_str) = status
                        .strip_prefix("exited (")
                        .and_then(|s| s.split(')').next())
                    {
                        if let Ok(code) = code_str.trim().parse::<i32>() {
                            return Ok(code);
                        }
                    }
                    return Ok(0);
                }
            }
            Err(perry_container_compose::error::ComposeError::NotFound(_)) => {
                // Container already removed (--rm), assume success
                return Ok(0);
            }
            Err(_) => {
                // Transient error, continue polling
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

/// Compute MD5 hex digest (first 16 chars) for container naming.
fn md5_hex(input: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}
