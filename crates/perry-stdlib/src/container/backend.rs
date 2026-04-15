//! Backend abstraction for container runtimes.
//!
//! Platform-adaptive selection:
//! - macOS / iOS  → AppleContainerBackend (wraps perry-container-compose AppleContainerBackend)
//! - All others   → PodmanBackend

use super::types::{
    ContainerError, ContainerHandle, ContainerInfo, ContainerLogs, ContainerSpec, ImageInfo,
};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tokio::process::Command;

// ─── ContainerBackend trait ───────────────────────────────────────────────────

#[async_trait]
pub trait ContainerBackend: Send + Sync {
    fn name(&self) -> &'static str;
    async fn check_available(&self) -> Result<(), ContainerError>;

    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle, ContainerError>;
    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle, ContainerError>;
    async fn start(&self, id: &str) -> Result<(), ContainerError>;
    async fn stop(&self, id: &str, timeout: u32) -> Result<(), ContainerError>;
    async fn remove(&self, id: &str, force: bool) -> Result<(), ContainerError>;
    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>, ContainerError>;
    async fn inspect(&self, id: &str) -> Result<ContainerInfo, ContainerError>;
    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs, ContainerError>;
    async fn exec(
        &self,
        id: &str,
        cmd: &[String],
        env: Option<&[(String, String)]>,
    ) -> Result<ContainerLogs, ContainerError>;
    async fn pull_image(&self, reference: &str) -> Result<(), ContainerError>;
    async fn list_images(&self) -> Result<Vec<ImageInfo>, ContainerError>;
    async fn remove_image(&self, reference: &str, force: bool) -> Result<(), ContainerError>;

    // ── Network operations ──

    /// Create a network with optional driver and labels.
    async fn create_network(
        &self,
        name: &str,
        driver: Option<&str>,
        labels: Option<&[(String, String)]>,
    ) -> Result<(), ContainerError>;

    /// Remove a network (idempotent — "not found" is OK).
    async fn remove_network(&self, name: &str) -> Result<(), ContainerError>;

    // ── Volume operations ──

    /// Create a named volume with optional driver and labels.
    async fn create_volume(
        &self,
        name: &str,
        driver: Option<&str>,
        labels: Option<&[(String, String)]>,
    ) -> Result<(), ContainerError>;

    /// Remove a named volume (idempotent — "not found" is OK).
    async fn remove_volume(&self, name: &str) -> Result<(), ContainerError>;
}

// ─── AppleContainerBackend ────────────────────────────────────────────────────
//
// On macOS / iOS this delegates to the `container` CLI via the same helper
// that `perry-container-compose` uses (its `AppleContainerBackend`), so there
// is exactly ONE place where CLI invocations live.
//
// The `perry-stdlib` backend simply adapts between the two type systems.

#[cfg(target_os = "macos")]
pub struct AppleContainerBackend {
    inner: perry_container_compose::backend::AppleContainerBackend,
}

#[cfg(target_os = "macos")]
impl AppleContainerBackend {
    pub fn new() -> Self {
        Self {
            inner: perry_container_compose::backend::AppleContainerBackend::new(),
        }
    }
}

#[cfg(target_os = "macos")]
#[async_trait]
impl ContainerBackend for AppleContainerBackend {
    fn name(&self) -> &'static str {
        "apple/container"
    }

    async fn check_available(&self) -> Result<(), ContainerError> {
        // Try running `container --version`
        Command::new("container")
            .arg("--version")
            .output()
            .await
            .map(|_| ())
            .map_err(|e| ContainerError::BackendError {
                code: 1,
                message: format!("apple/container binary not found: {}", e),
            })
    }

    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle, ContainerError> {
        use perry_container_compose::backend::Backend;
        use std::collections::HashMap;

        let env: HashMap<String, String> = spec.env.clone().unwrap_or_default();
        let ports: Vec<String> = spec.ports.clone().unwrap_or_default();
        let volumes: Vec<String> = spec.volumes.clone().unwrap_or_default();

        self.inner
            .run(
                &spec.image,
                spec.name.as_deref().unwrap_or(""),
                if ports.is_empty() { None } else { Some(&ports) },
                if env.is_empty() { None } else { Some(&env) },
                if volumes.is_empty() { None } else { Some(&volumes) },
                None,
                spec.cmd.as_deref(),
                true, // detach
            )
            .await
            .map(|_| ContainerHandle {
                id: spec.name.clone().unwrap_or_default(),
                name: spec.name.clone(),
            })
            .map_err(map_compose_err)
    }

    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle, ContainerError> {
        // Apple Container doesn't have a separate create; run detached then stop.
        let handle = self.run(spec).await?;
        let _ = self.stop(&handle.id, 0).await;
        Ok(handle)
    }

    async fn start(&self, id: &str) -> Result<(), ContainerError> {
        use perry_container_compose::backend::Backend;
        self.inner.start(id).await.map_err(map_compose_err)
    }

    async fn stop(&self, id: &str, _timeout: u32) -> Result<(), ContainerError> {
        use perry_container_compose::backend::Backend;
        self.inner.stop(id).await.map_err(map_compose_err)
    }

    async fn remove(&self, id: &str, force: bool) -> Result<(), ContainerError> {
        use perry_container_compose::backend::Backend;
        self.inner.remove(id, force).await.map_err(map_compose_err)
    }

    async fn list(&self, _all: bool) -> Result<Vec<ContainerInfo>, ContainerError> {
        use perry_container_compose::backend::Backend;
        let infos = self
            .inner
            .list(None)
            .await
            .map_err(map_compose_err)?;
        Ok(infos.into_iter().map(compose_info_to_stdlib).collect())
    }

    async fn inspect(&self, id: &str) -> Result<ContainerInfo, ContainerError> {
        use perry_container_compose::backend::Backend;
        use perry_container_compose::commands::ContainerStatus;

        let status = self.inner.inspect(id).await.map_err(map_compose_err)?;
        Ok(ContainerInfo {
            id: id.to_string(),
            name: id.to_string(),
            image: String::new(),
            status: match status {
                ContainerStatus::Running => "running".to_string(),
                ContainerStatus::Stopped => "exited".to_string(),
                ContainerStatus::NotFound => {
                    return Err(ContainerError::NotFound(id.to_string()))
                }
            },
            ports: Vec::new(),
            created: String::new(),
        })
    }

    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs, ContainerError> {
        use perry_container_compose::backend::Backend;
        let stdout = self
            .inner
            .logs(id, tail, false)
            .await
            .map_err(map_compose_err)?;
        Ok(ContainerLogs {
            stdout,
            stderr: String::new(),
        })
    }

    async fn exec(
        &self,
        id: &str,
        cmd: &[String],
        env: Option<&[(String, String)]>,
    ) -> Result<ContainerLogs, ContainerError> {
        use perry_container_compose::backend::Backend;
        let env_map: Option<std::collections::HashMap<String, String>> = env.map(|pairs| {
            pairs.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
        });
        let result = self
            .inner
            .exec(id, cmd, None, None, env_map.as_ref())
            .await
            .map_err(map_compose_err)?;
        Ok(ContainerLogs {
            stdout: result.stdout,
            stderr: result.stderr,
        })
    }

    async fn pull_image(&self, reference: &str) -> Result<(), ContainerError> {
        // `container pull <reference>`
        let output = Command::new("container")
            .args(["pull", reference])
            .output()
            .await
            .map_err(|e| ContainerError::BackendError {
                code: 1,
                message: e.to_string(),
            })?;
        if output.status.success() {
            Ok(())
        } else {
            Err(ContainerError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: String::from_utf8_lossy(&output.stderr).to_string(),
            })
        }
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>, ContainerError> {
        let output = Command::new("container")
            .args(["images", "--format", "json"])
            .output()
            .await
            .map_err(|e| ContainerError::BackendError {
                code: 1,
                message: e.to_string(),
            })?;

        if !output.status.success() {
            return Err(ContainerError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        let json: Value =
            serde_json::from_slice(&output.stdout).unwrap_or(Value::Array(vec![]));
        let images = json.as_array().map(|v| v.as_slice()).unwrap_or(&[]);
        Ok(images.iter().filter_map(parse_image_info).collect())
    }

    async fn remove_image(&self, reference: &str, force: bool) -> Result<(), ContainerError> {
        let mut args = vec!["rmi"];
        if force {
            args.push("-f");
        }
        args.push(reference);

        let output = Command::new("container")
            .args(&args)
            .output()
            .await
            .map_err(|e| ContainerError::BackendError {
                code: 1,
                message: e.to_string(),
            })?;

        if output.status.success() {
            Ok(())
        } else {
            Err(ContainerError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: String::from_utf8_lossy(&output.stderr).to_string(),
            })
        }
    }

    // ── Network operations ──

    async fn create_network(
        &self,
        name: &str,
        driver: Option<&str>,
        labels: Option<&[(String, String)]>,
    ) -> Result<(), ContainerError> {
        use perry_container_compose::backend::Backend;
        let labels_map: Option<std::collections::HashMap<String, String>> =
            labels.map(|pairs| pairs.iter().cloned().collect());
        self.inner
            .create_network(name, driver, labels_map.as_ref())
            .await
            .map_err(map_compose_err)
    }

    async fn remove_network(&self, name: &str) -> Result<(), ContainerError> {
        use perry_container_compose::backend::Backend;
        self.inner.remove_network(name).await.map_err(map_compose_err)
    }

    // ── Volume operations ──

    async fn create_volume(
        &self,
        name: &str,
        driver: Option<&str>,
        labels: Option<&[(String, String)]>,
    ) -> Result<(), ContainerError> {
        use perry_container_compose::backend::Backend;
        let labels_map: Option<std::collections::HashMap<String, String>> =
            labels.map(|pairs| pairs.iter().cloned().collect());
        self.inner
            .create_volume(name, driver, labels_map.as_ref())
            .await
            .map_err(map_compose_err)
    }

    async fn remove_volume(&self, name: &str) -> Result<(), ContainerError> {
        use perry_container_compose::backend::Backend;
        self.inner.remove_volume(name).await.map_err(map_compose_err)
    }
}

// ─── PodmanBackend ────────────────────────────────────────────────────────────

pub struct PodmanBackend;

impl PodmanBackend {
    pub fn new() -> Self {
        Self
    }

    fn find_binary() -> Option<String> {
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

#[async_trait]
impl ContainerBackend for PodmanBackend {
    fn name(&self) -> &'static str {
        "podman"
    }

    async fn check_available(&self) -> Result<(), ContainerError> {
        if let Some(binary) = Self::find_binary() {
            Command::new(&binary)
                .arg("--version")
                .output()
                .await
                .map(|_| ())
                .map_err(|e| ContainerError::BackendError {
                    code: 1,
                    message: format!("Failed to execute podman: {}", e),
                })
        } else {
            Err(ContainerError::BackendError {
                code: 1,
                message: "podman binary not found. Please install podman.".to_string(),
            })
        }
    }

    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle, ContainerError> {
        let binary = Self::find_binary().ok_or_else(|| ContainerError::BackendError {
            code: 1,
            message: "podman binary not found".to_string(),
        })?;

        let mut cmd = Command::new(&binary);
        cmd.arg("run").arg("-d");

        if let Some(name) = &spec.name {
            cmd.arg("--name").arg(name);
        }
        if let Some(ports) = &spec.ports {
            for p in ports {
                cmd.arg("-p").arg(p);
            }
        }
        if let Some(vols) = &spec.volumes {
            for v in vols {
                cmd.arg("-v").arg(v);
            }
        }
        if let Some(env) = &spec.env {
            for (k, v) in env {
                cmd.arg("-e").arg(format!("{}={}", k, v));
            }
        }
        if spec.rm.unwrap_or(false) {
            cmd.arg("--rm");
        }
        cmd.arg(&spec.image);

        let output = execute_cmd(&mut cmd).await?;
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
        cmd.arg("create").arg(&spec.image);
        let output = execute_cmd(&mut cmd).await?;
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
        let output = execute_cmd(&mut cmd).await?;
        require_success(output)
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
        let output = execute_cmd(&mut cmd).await?;
        require_success(output)
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
        let output = execute_cmd(&mut cmd).await?;
        require_success(output)
    }

    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>, ContainerError> {
        let binary = Self::find_binary().ok_or_else(|| ContainerError::BackendError {
            code: 1,
            message: "podman binary not found".to_string(),
        })?;
        let mut cmd = Command::new(&binary);
        cmd.arg("ps").arg("--format").arg("json");
        if all {
            cmd.arg("-a");
        }
        let output = execute_cmd(&mut cmd).await?;
        if !output.status.success() {
            return Err(ContainerError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }
        let json: Value = serde_json::from_slice(&output.stdout).unwrap_or(Value::Array(vec![]));
        let items = json.as_array().map(|v| v.as_slice()).unwrap_or(&[]);
        Ok(items
            .iter()
            .filter_map(|v| parse_podman_container_info(v).ok())
            .collect())
    }

    async fn inspect(&self, id: &str) -> Result<ContainerInfo, ContainerError> {
        let binary = Self::find_binary().ok_or_else(|| ContainerError::BackendError {
            code: 1,
            message: "podman binary not found".to_string(),
        })?;
        let mut cmd = Command::new(&binary);
        cmd.arg("inspect").arg("--format").arg("json").arg(id);
        let output = execute_cmd(&mut cmd).await?;
        if !output.status.success() {
            return Err(ContainerError::NotFound(id.to_string()));
        }
        let json: Value = serde_json::from_slice(&output.stdout).map_err(|e| {
            ContainerError::BackendError {
                code: 1,
                message: format!("Failed to parse inspect JSON: {}", e),
            }
        })?;
        let first = json
            .as_array()
            .and_then(|a| a.first())
            .ok_or_else(|| ContainerError::NotFound(id.to_string()))?;
        parse_podman_container_info(first)
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
        let output = execute_cmd(&mut cmd).await?;
        Ok(ContainerLogs {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }

    async fn exec(
        &self,
        id: &str,
        cmd: &[String],
        env: Option<&[(String, String)]>,
    ) -> Result<ContainerLogs, ContainerError> {
        let binary = Self::find_binary().ok_or_else(|| ContainerError::BackendError {
            code: 1,
            message: "podman binary not found".to_string(),
        })?;
        let mut command = Command::new(&binary);
        command.arg("exec");
        if let Some(pairs) = env {
            for (k, v) in pairs {
                command.arg("-e").arg(format!("{}={}", k, v));
            }
        }
        command.arg(id);
        for arg in cmd {
            command.arg(arg);
        }
        let output = execute_cmd(&mut command).await?;
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
        let output = execute_cmd(&mut cmd).await?;
        require_success(output)
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>, ContainerError> {
        let binary = Self::find_binary().ok_or_else(|| ContainerError::BackendError {
            code: 1,
            message: "podman binary not found".to_string(),
        })?;
        let mut cmd = Command::new(&binary);
        cmd.arg("images").arg("--format").arg("json");
        let output = execute_cmd(&mut cmd).await?;
        if !output.status.success() {
            return Err(ContainerError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }
        let json: Value = serde_json::from_slice(&output.stdout).unwrap_or(Value::Array(vec![]));
        let items = json.as_array().map(|v| v.as_slice()).unwrap_or(&[]);
        Ok(items.iter().filter_map(parse_image_info).collect())
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
        let output = execute_cmd(&mut cmd).await?;
        require_success(output)
    }

    // ── Network operations ──

    async fn create_network(
        &self,
        name: &str,
        driver: Option<&str>,
        labels: Option<&[(String, String)]>,
    ) -> Result<(), ContainerError> {
        let binary = Self::find_binary().ok_or_else(|| ContainerError::BackendError {
            code: 1,
            message: "podman binary not found".to_string(),
        })?;
        let mut cmd = Command::new(&binary);
        cmd.args(["network", "create"]);
        if let Some(d) = driver {
            cmd.arg("--driver").arg(d);
        }
        if let Some(pairs) = labels {
            for (k, v) in pairs {
                cmd.arg("--label").arg(format!("{}={}", k, v));
            }
        }
        cmd.arg(name);
        let output = execute_cmd(&mut cmd).await?;
        require_success(output)
    }

    async fn remove_network(&self, name: &str) -> Result<(), ContainerError> {
        let binary = Self::find_binary().ok_or_else(|| ContainerError::BackendError {
            code: 1,
            message: "podman binary not found".to_string(),
        })?;
        let mut cmd = Command::new(&binary);
        cmd.args(["network", "rm", name]);
        let output = execute_cmd(&mut cmd).await?;
        // Idempotent: ignore "not found"
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("not found")
                || stderr.contains("no such")
                || stderr.contains("does not exist")
            {
                return Ok(());
            }
            return Err(ContainerError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: stderr.to_string(),
            });
        }
        Ok(())
    }

    // ── Volume operations ──

    async fn create_volume(
        &self,
        name: &str,
        driver: Option<&str>,
        labels: Option<&[(String, String)]>,
    ) -> Result<(), ContainerError> {
        let binary = Self::find_binary().ok_or_else(|| ContainerError::BackendError {
            code: 1,
            message: "podman binary not found".to_string(),
        })?;
        let mut cmd = Command::new(&binary);
        cmd.args(["volume", "create"]);
        if let Some(d) = driver {
            cmd.arg("--driver").arg(d);
        }
        if let Some(pairs) = labels {
            for (k, v) in pairs {
                cmd.arg("--label").arg(format!("{}={}", k, v));
            }
        }
        cmd.arg(name);
        let output = execute_cmd(&mut cmd).await?;
        require_success(output)
    }

    async fn remove_volume(&self, name: &str) -> Result<(), ContainerError> {
        let binary = Self::find_binary().ok_or_else(|| ContainerError::BackendError {
            code: 1,
            message: "podman binary not found".to_string(),
        })?;
        let mut cmd = Command::new(&binary);
        cmd.args(["volume", "rm", name]);
        let output = execute_cmd(&mut cmd).await?;
        // Idempotent: ignore "not found"
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("not found")
                || stderr.contains("no such")
                || stderr.contains("does not exist")
            {
                return Ok(());
            }
            return Err(ContainerError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: stderr.to_string(),
            });
        }
        Ok(())
    }
}

// ─── Backend selection ────────────────────────────────────────────────────────

pub fn get_backend() -> Result<Arc<dyn ContainerBackend>, ContainerError> {
    let backend: Arc<dyn ContainerBackend> = match std::env::consts::OS {
        #[cfg(target_os = "macos")]
        "macos" | "ios" => Arc::new(AppleContainerBackend::new()),
        #[cfg(not(target_os = "macos"))]
        "macos" | "ios" => Arc::new(PodmanBackend::new()), // fallback on non-mac builds
        _ => Arc::new(PodmanBackend::new()),
    };
    Ok(backend)
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

async fn execute_cmd(cmd: &mut Command) -> Result<std::process::Output, ContainerError> {
    cmd.output().await.map_err(|e| ContainerError::BackendError {
        code: 1,
        message: format!("Failed to execute backend command: {}", e),
    })
}

fn require_success(output: std::process::Output) -> Result<(), ContainerError> {
    if output.status.success() {
        Ok(())
    } else {
        Err(ContainerError::BackendError {
            code: output.status.code().unwrap_or(-1),
            message: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}

#[cfg(target_os = "macos")]
fn map_compose_err(e: perry_container_compose::error::ComposeError) -> ContainerError {
    ContainerError::BackendError {
        code: -1,
        message: e.to_string(),
    }
}

#[cfg(target_os = "macos")]
fn compose_info_to_stdlib(
    info: perry_container_compose::backend::ContainerInfo,
) -> ContainerInfo {
    ContainerInfo {
        id: info.id,
        name: info.name,
        image: info.image,
        status: info.status,
        ports: info.ports,
        created: info.created,
    }
}

fn parse_podman_container_info(json: &Value) -> Result<ContainerInfo, ContainerError> {
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
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect(),
        created: json["Created"].as_str().unwrap_or("").to_string(),
    })
}

fn parse_image_info(json: &Value) -> Option<ImageInfo> {
    Some(ImageInfo {
        id: json["Id"].as_str()?.to_string(),
        repository: json["Repository"].as_str().unwrap_or("").to_string(),
        tag: json["Tag"].as_str().unwrap_or("").to_string(),
        size: json["Size"].as_u64().unwrap_or(0),
        created: json["Created"].as_str().unwrap_or("").to_string(),
    })
}
