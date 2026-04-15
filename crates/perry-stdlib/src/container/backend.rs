//! Backend abstraction for container runtimes
//!
//! Provides platform-adaptive backend selection:
//! - macOS/iOS: apple/container
//! - All others: podman

use super::types::{ContainerError, ContainerHandle, ContainerInfo, ContainerLogs, ContainerSpec, ImageInfo};
use async_trait::async_trait;
use serde_json::Value;
use std::process::Output;
use std::time::Duration;
use std::sync::Arc;
use tokio::process::Command;

/// Trait for container backend implementations
#[async_trait::async_trait]
pub trait ContainerBackend: Send + Sync {
    /// Get backend name (for display and debugging)
    fn name(&self) -> &'static str;

    /// Check if backend binary is available
    async fn check_available(&self) -> Result<(), ContainerError>;

    /// Run a container
    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle, ContainerError>;

    /// Create a container without starting it
    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle, ContainerError>;

    /// Start a container
    async fn start(&self, id: &str) -> Result<(), ContainerError>;

    /// Stop a container
    async fn stop(&self, id: &str, timeout: u32) -> Result<(), ContainerError>;

    /// Remove a container
    async fn remove(&self, id: &str, force: bool) -> Result<(), ContainerError>;

    /// List containers
    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>, ContainerError>;

    /// Inspect a container
    async fn inspect(&self, id: &str) -> Result<ContainerInfo, ContainerError>;

    /// Get container logs
    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs, ContainerError>;

    /// Execute a command in a container
    async fn exec(
        &self,
        id: &str,
        cmd: &[String],
        env: Option<&[(String, String)]>,
    ) -> Result<ContainerLogs, ContainerError>;

    /// Pull an image
    async fn pull_image(&self, reference: &str) -> Result<(), ContainerError>;

    /// List images
    async fn list_images(&self) -> Result<Vec<ImageInfo>, ContainerError>;

    /// Remove an image
    async fn remove_image(&self, reference: &str, force: bool) -> Result<(), ContainerError>;
}

/// Podman backend implementation
pub struct PodmanBackend;

impl PodmanBackend {
    /// Create new Podman backend
    pub fn new() -> Self {
        Self
    }

    /// Find podman binary
    fn find_binary() -> Option<String> {
        // Check common paths
        let paths = [
            "podman",
            "/usr/local/bin/podman",
            "/usr/bin/podman",
            "/opt/homebrew/bin/podman",
        ];

        for path in &paths {
            if std::path::Path::new(path).exists() {
                return Some(path.to_string());
            }
        }

        None
    }
}

#[async_trait::async_trait]
impl ContainerBackend for PodmanBackend {
    fn name(&self) -> &'static str {
        "podman"
    }

    async fn check_available(&self) -> Result<(), ContainerError> {
        if let Some(binary) = Self::find_binary() {
            // Try to run podman --version
            let output = Command::new(&binary)
                .arg("--version")
                .output()
                .await;

            match output {
                Ok(_) => Ok(()),
                Err(e) => Err(ContainerError::BackendError {
                    code: 1,
                    message: format!("Failed to execute podman: {}", e),
                }),
            }
        } else {
            Err(ContainerError::BackendError {
                code: 1,
                message: "podman binary not found. Please install podman to use container functionality.".to_string(),
            })
        }
    }

    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle, ContainerError> {
        let binary = Self::find_binary().ok_or_else(|| ContainerError::BackendError {
            code: 1,
            message: "podman binary not found".to_string(),
        })?;

        let mut cmd = Command::new(&binary);
        cmd.arg("run")
            .arg("-d"); // detached mode

        // Add name if specified
        if let Some(name) = &spec.name {
            cmd.arg("--name").arg(name);
        }

        // Add ports
        if let Some(ports) = &spec.ports {
            for port in ports {
                cmd.arg("-p").arg(port);
            }
        }

        // Add volumes
        if let Some(volumes) = &spec.volumes {
            for vol in volumes {
                cmd.arg("-v").arg(vol);
            }
        }

        // Add environment variables
        if let Some(env) = &spec.env {
            for (k, v) in env {
                cmd.arg("-e").arg(format!("{}={}", k, v));
            }
        }

        // Add rm flag if specified
        if spec.rm.unwrap_or(false) {
            cmd.arg("--rm");
        }

        // Add image
        cmd.arg(&spec.image);

        // Run command
        let output = execute_backend_command(&mut cmd).await?;

        // Parse container ID from output
        let id = String::from_utf8_lossy(&output.stdout).trim().to_string();

        if id.is_empty() {
            return Err(ContainerError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        Ok(ContainerHandle {
            id,
            name: spec.name.clone(),
        })
    }

    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle, ContainerError> {
        let binary = Self::find_binary().ok_or_else(|| ContainerError::BackendError {
            code: 1,
            message: "podman binary not found".to_string(),
        })?;

        let mut cmd = Command::new(&binary);
        cmd.arg("create");

        // Same options as run but without -d
        // (simplified for now - will add full support in later tasks)

        cmd.arg(&spec.image);

        let output = execute_backend_command(&mut cmd).await?;

        let id = String::from_utf8_lossy(&output.stdout).trim().to_string();

        if id.is_empty() {
            return Err(ContainerError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        Ok(ContainerHandle {
            id,
            name: spec.name.clone(),
        })
    }

    async fn start(&self, id: &str) -> Result<(), ContainerError> {
        let binary = Self::find_binary().ok_or_else(|| ContainerError::BackendError {
            code: 1,
            message: "podman binary not found".to_string(),
        })?;

        let mut cmd = Command::new(&binary);
        cmd.arg("start").arg(id);

        let output = execute_backend_command(&mut cmd).await?;

        if !output.status.success() {
            return Err(ContainerError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        Ok(())
    }

    async fn stop(&self, id: &str, timeout: u32) -> Result<(), ContainerError> {
        let binary = Self::find_binary().ok_or_else(|| ContainerError::BackendError {
            code: 1,
            message: "podman binary not found".to_string(),
        })?;

        let mut cmd = Command::new(&binary);
        cmd.arg("stop")
            .arg(format!("--time={}", timeout))
            .arg(id);

        let output = execute_backend_command(&mut cmd).await?;

        if !output.status.success() {
            return Err(ContainerError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        Ok(())
    }

    async fn remove(&self, id: &str, force: bool) -> Result<(), ContainerError> {
        let binary = Self::find_binary().ok_or_else(|| ContainerError::BackendError {
            code: 1,
            message: "podman binary not found".to_string(),
        })?;

        let mut cmd = Command::new(&binary);
        cmd.arg("rm");

        if force {
            cmd.arg("-f");
        }

        cmd.arg(id);

        let output = execute_backend_command(&mut cmd).await?;

        if !output.status.success() {
            return Err(ContainerError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        Ok(())
    }

    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>, ContainerError> {
        let binary = Self::find_binary().ok_or_else(|| ContainerError::BackendError {
            code: 1,
            message: "podman binary not found".to_string(),
        })?;

        let mut cmd = Command::new(&binary);
        cmd.arg("ps")
            .arg("--format")  // Use JSON format
            .arg("json");

        if all {
            cmd.arg("-a");
        }

        let output = execute_backend_command(&mut cmd).await?;

        if !output.status.success() {
            return Err(ContainerError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        // Parse JSON output
        let json: Value = serde_json::from_slice(&output.stdout)
            .map_err(|e| ContainerError::BackendError {
                code: 1,
                message: format!("Failed to parse container list JSON: {}", e),
            })?;

        let containers = json.as_array()
            .ok_or_else(|| ContainerError::BackendError {
                code: 1,
                message: "Invalid container list format".to_string(),
            })?;

        // Convert to ContainerInfo
        let mut result = Vec::new();
        for c in containers {
            if let Ok(info) = parse_container_info_from_json(c) {
                result.push(info);
            }
        }

        Ok(result)
    }

    async fn inspect(&self, id: &str) -> Result<ContainerInfo, ContainerError> {
        let binary = Self::find_binary().ok_or_else(|| ContainerError::BackendError {
            code: 1,
            message: "podman binary not found".to_string(),
        })?;

        let mut cmd = Command::new(&binary);
        cmd.arg("inspect")
            .arg("--format")
            .arg("json")
            .arg(id);

        let output = execute_backend_command(&mut cmd).await?;

        if !output.status.success() {
            return Err(ContainerError::NotFound(id.to_string()));
        }

        let json: Value = serde_json::from_slice(&output.stdout)
            .map_err(|e| ContainerError::BackendError {
                code: 1,
                message: format!("Failed to parse inspect JSON: {}", e),
            })?;

        let array = json.as_array()
            .ok_or_else(|| ContainerError::BackendError {
                code: 1,
                message: "Invalid inspect format".to_string(),
            })?;

        let first = array.first()
            .ok_or_else(|| ContainerError::NotFound(id.to_string()))?;

        parse_container_info_from_json(first)
    }

    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs, ContainerError> {
        let binary = Self::find_binary().ok_or_else(|| ContainerError::BackendError {
            code: 1,
            message: "podman binary not found".to_string(),
        })?;

        let mut cmd = Command::new(&binary);
        cmd.arg("logs");

        if let Some(n) = tail {
            cmd.arg("--tail").arg(n.to_string());
        }

        cmd.arg(id);

        let output = execute_backend_command(&mut cmd).await?;

        // Podman doesn't separate stdout/stderr in the same way
        // For now, we'll put everything in stdout
        let logs = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(ContainerLogs {
            stdout: logs,
            stderr,
        })
    }

    async fn exec(
        &self,
        id: &str,
        cmd: &[String],
        _env: Option<&[(String, String)]>,
    ) -> Result<ContainerLogs, ContainerError> {
        let binary = Self::find_binary().ok_or_else(|| ContainerError::BackendError {
            code: 1,
            message: "podman binary not found".to_string(),
        })?;

        let mut command_cmd = Command::new(&binary);
        command_cmd.arg("exec").arg(id);

        for arg in cmd {
            command_cmd.arg(arg);
        }

        let output = execute_backend_command(&mut command_cmd).await?;

        Ok(ContainerLogs {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }

    async fn pull_image(&self, reference: &str) -> Result<(), ContainerError> {
        let binary = Self::find_binary().ok_or_else(|| ContainerError::BackendError {
            code: 1,
            message: "podman binary not found".to_string(),
        })?;

        let mut cmd = Command::new(&binary);
        cmd.arg("pull").arg(reference);

        let output = execute_backend_command(&mut cmd).await?;

        if !output.status.success() {
            return Err(ContainerError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        Ok(())
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>, ContainerError> {
        let binary = Self::find_binary().ok_or_else(|| ContainerError::BackendError {
            code: 1,
            message: "podman binary not found".to_string(),
        })?;

        let mut cmd = Command::new(&binary);
        cmd.arg("images")
            .arg("--format")
            .arg("json");

        let output = execute_backend_command(&mut cmd).await?;

        if !output.status.success() {
            return Err(ContainerError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        let json: Value = serde_json::from_slice(&output.stdout)
            .map_err(|e| ContainerError::BackendError {
                code: 1,
                message: format!("Failed to parse image list JSON: {}", e),
            })?;

        let images = json.as_array()
            .ok_or_else(|| ContainerError::BackendError {
                code: 1,
                message: "Invalid image list format".to_string(),
            })?;

        let mut result = Vec::new();
        for img in images {
            if let Ok(info) = parse_image_info_from_json(img) {
                result.push(info);
            }
        }

        Ok(result)
    }

    async fn remove_image(&self, reference: &str, force: bool) -> Result<(), ContainerError> {
        let binary = Self::find_binary().ok_or_else(|| ContainerError::BackendError {
            code: 1,
            message: "podman binary not found".to_string(),
        })?;

        let mut cmd = Command::new(&binary);
        cmd.arg("rmi");

        if force {
            cmd.arg("-f");
        }

        cmd.arg(reference);

        let output = execute_backend_command(&mut cmd).await?;

        if !output.status.success() {
            return Err(ContainerError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        Ok(())
    }
}

/// Apple Container backend implementation (placeholder for macOS/iOS)
pub struct AppleContainerBackend;

impl AppleContainerBackend {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl ContainerBackend for AppleContainerBackend {
    fn name(&self) -> &'static str {
        "apple/container"
    }

    async fn check_available(&self) -> Result<(), ContainerError> {
        // TODO: Implement apple/container backend
        Err(ContainerError::BackendError {
            code: 1,
            message: "apple/container backend not yet implemented".to_string(),
        })
    }

    async fn run(&self, _spec: &ContainerSpec) -> Result<ContainerHandle, ContainerError> {
        Err(ContainerError::BackendError {
            code: 1,
            message: "apple/container backend not yet implemented".to_string(),
        })
    }

    async fn create(&self, _spec: &ContainerSpec) -> Result<ContainerHandle, ContainerError> {
        Err(ContainerError::BackendError {
            code: 1,
            message: "apple/container backend not yet implemented".to_string(),
        })
    }

    async fn start(&self, _id: &str) -> Result<(), ContainerError> {
        Err(ContainerError::BackendError {
            code: 1,
            message: "apple/container backend not yet implemented".to_string(),
        })
    }

    async fn stop(&self, _id: &str, _timeout: u32) -> Result<(), ContainerError> {
        Err(ContainerError::BackendError {
            code: 1,
            message: "apple/container backend not yet implemented".to_string(),
        })
    }

    async fn remove(&self, _id: &str, _force: bool) -> Result<(), ContainerError> {
        Err(ContainerError::BackendError {
            code: 1,
            message: "apple/container backend not yet implemented".to_string(),
        })
    }

    async fn list(&self, _all: bool) -> Result<Vec<ContainerInfo>, ContainerError> {
        Err(ContainerError::BackendError {
            code: 1,
            message: "apple/container backend not yet implemented".to_string(),
        })
    }

    async fn inspect(&self, _id: &str) -> Result<ContainerInfo, ContainerError> {
        Err(ContainerError::BackendError {
            code: 1,
            message: "apple/container backend not yet implemented".to_string(),
        })
    }

    async fn logs(&self, _id: &str, _tail: Option<u32>) -> Result<ContainerLogs, ContainerError> {
        Err(ContainerError::BackendError {
            code: 1,
            message: "apple/container backend not yet implemented".to_string(),
        })
    }

    async fn exec(
        &self,
        _id: &str,
        _cmd: &[String],
        _env: Option<&[(String, String)]>,
    ) -> Result<ContainerLogs, ContainerError> {
        Err(ContainerError::BackendError {
            code: 1,
            message: "apple/container backend not yet implemented".to_string(),
        })
    }

    async fn pull_image(&self, _reference: &str) -> Result<(), ContainerError> {
        Err(ContainerError::BackendError {
            code: 1,
            message: "apple/container backend not yet implemented".to_string(),
        })
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>, ContainerError> {
        Err(ContainerError::BackendError {
            code: 1,
            message: "apple/container backend not yet implemented".to_string(),
        })
    }

    async fn remove_image(&self, _reference: &str, _force: bool) -> Result<(), ContainerError> {
        Err(ContainerError::BackendError {
            code: 1,
            message: "apple/container backend not yet implemented".to_string(),
        })
    }
}

// ============ Helper Functions ============

/// Get the appropriate backend for the current platform
pub fn get_backend() -> Result<Arc<dyn ContainerBackend>, ContainerError> {
    let os = std::env::consts::OS;

    let backend: Arc<dyn ContainerBackend> = match os {
        "macos" | "ios" => Arc::new(AppleContainerBackend::new()),
        _ => Arc::new(PodmanBackend::new()),
    };

    Ok(backend)
}

/// Execute a backend command and return the output
async fn execute_backend_command(cmd: &mut Command) -> Result<Output, ContainerError> {
    let output = cmd
        .output()
        .await
        .map_err(|e| ContainerError::BackendError {
            code: 1,
            message: format!("Failed to execute backend command: {}", e),
        })?;

    Ok(output)
}

/// Parse ContainerInfo from JSON value
fn parse_container_info_from_json(json: &Value) -> Result<ContainerInfo, ContainerError> {
    Ok(ContainerInfo {
        id: json["Id"].as_str().unwrap_or("").to_string(),
        name: json["Names"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        image: json["Image"].as_str().unwrap_or("").to_string(),
        status: json["Status"].as_str().unwrap_or("").to_string(),
        ports: json["Ports"]
            .as_str()
            .unwrap_or("")
            .split(", ")
            .map(|s| s.to_string())
            .collect(),
        created: json["Created"].as_str().unwrap_or("").to_string(),
    })
}

/// Parse ImageInfo from JSON value
fn parse_image_info_from_json(json: &Value) -> Result<ImageInfo, ContainerError> {
    Ok(ImageInfo {
        id: json["Id"].as_str().unwrap_or("").to_string(),
        repository: json["Repository"].as_str().unwrap_or("").to_string(),
        tag: json["Tag"].as_str().unwrap_or("").to_string(),
        size: json["Size"].as_u64().unwrap_or(0),
        created: json["Created"].as_str().unwrap_or("").to_string(),
    })
}
