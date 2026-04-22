//! Backend abstraction for container runtimes.
//!
//! Platform-adaptive selection:
//! - macOS / iOS  → AppleContainerBackend (wraps perry-container-compose AppleContainerBackend)
//! - All others   → PodmanBackend
//!
//! The `ContainerBackend` trait mirrors the signature of
//! `perry_container_compose::backend::ContainerBackend` so that the
//! `AppleContainerBackend` adapter is nearly zero-cost.

use super::types::{
    ComposeNetwork, ComposeVolume, ContainerError, ContainerHandle, ContainerInfo,
    ContainerLogs, ContainerSpec, ImageInfo,
};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use tokio::process::Command;

static GLOBAL_BACKEND: OnceLock<Arc<dyn ContainerBackend>> = OnceLock::new();

// ─── ContainerBackend trait ───────────────────────────────────────────────────
//
// Mirrors perry_container_compose::backend::ContainerBackend but uses the
// stdlib's own type aliases (serde_json-based) so the rest of the stdlib
// does not need to depend on serde_yaml.

#[async_trait]
pub trait ContainerBackend: Send + Sync {
    /// Backend name for display (e.g. "apple-container", "podman")
    fn name(&self) -> &'static str;

    /// Check whether the backend binary is available on PATH.
    async fn check_available(&self) -> Result<(), ContainerError>;

    /// Run a container (create + start). Returns a handle.
    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle, ContainerError>;

    /// Create a container (without starting it).
    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle, ContainerError>;

    /// Start an existing stopped container.
    async fn start(&self, id: &str) -> Result<(), ContainerError>;

    /// Stop a running container. `timeout` = seconds to wait before SIGKILL.
    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<(), ContainerError>;

    /// Remove a container.
    async fn remove(&self, id: &str, force: bool) -> Result<(), ContainerError>;

    /// List all containers.
    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>, ContainerError>;

    /// Inspect a container.
    async fn inspect(&self, id: &str) -> Result<ContainerInfo, ContainerError>;

    /// Fetch logs from a container.
    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs, ContainerError>;

    /// Execute a command inside a running container.
    async fn exec(
        &self,
        id: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Result<ContainerLogs, ContainerError>;

    /// Pull an image.
    async fn pull_image(&self, reference: &str) -> Result<(), ContainerError>;

    /// List images.
    async fn list_images(&self) -> Result<Vec<ImageInfo>, ContainerError>;

    /// Remove an image.
    async fn remove_image(&self, reference: &str, force: bool) -> Result<(), ContainerError>;

    // ── Network operations ──

    /// Create a network with full config.
    async fn create_network(
        &self,
        name: &str,
        config: &ComposeNetwork,
    ) -> Result<(), ContainerError>;

    /// Remove a network (idempotent — "not found" is OK).
    async fn remove_network(&self, name: &str) -> Result<(), ContainerError>;

    // ── Volume operations ──

    /// Create a named volume with full config.
    async fn create_volume(
        &self,
        name: &str,
        config: &ComposeVolume,
    ) -> Result<(), ContainerError>;

    /// Remove a named volume (idempotent — "not found" is OK).
    async fn remove_volume(&self, name: &str) -> Result<(), ContainerError>;
}

// ─── AppleContainerBackend ────────────────────────────────────────────────────
//
// On macOS / iOS this delegates to the `perry-container-compose` crate's
// `AppleContainerBackend` so CLI invocations live in exactly one place.
// The stdlib adapter only converts between the two type systems at the
// boundary.

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

/// Convert stdlib `ContainerSpec` → compose-crate `ContainerSpec`.
#[cfg(target_os = "macos")]
fn spec_to_compose(spec: &super::types::ContainerSpec) -> perry_container_compose::types::ContainerSpec {
    perry_container_compose::types::ContainerSpec {
        image: spec.image.clone(),
        name: spec.name.clone(),
        ports: spec.ports.clone(),
        volumes: spec.volumes.clone(),
        env: spec.env.clone(),
        cmd: spec.cmd.clone(),
        entrypoint: spec.entrypoint.clone(),
        network: spec.network.clone(),
        rm: spec.rm,
    }
}

#[cfg(target_os = "macos")]
#[async_trait]
impl ContainerBackend for AppleContainerBackend {
    fn name(&self) -> &'static str {
        "apple/container"
    }

    async fn check_available(&self) -> Result<(), ContainerError> {
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
        use perry_container_compose::backend::ContainerBackend as CCB;
        let cspec = spec_to_compose(spec);
        let h = CCB::run(&self.inner, &cspec).await.map_err(map_compose_err)?;
        Ok(ContainerHandle { id: h.id, name: h.name })
    }

    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle, ContainerError> {
        use perry_container_compose::backend::ContainerBackend as CCB;
        let cspec = spec_to_compose(spec);
        let h = CCB::create(&self.inner, &cspec).await.map_err(map_compose_err)?;
        Ok(ContainerHandle { id: h.id, name: h.name })
    }

    async fn start(&self, id: &str) -> Result<(), ContainerError> {
        use perry_container_compose::backend::ContainerBackend as CCB;
        CCB::start(&self.inner, id).await.map_err(map_compose_err)
    }

    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<(), ContainerError> {
        use perry_container_compose::backend::ContainerBackend as CCB;
        CCB::stop(&self.inner, id, timeout).await.map_err(map_compose_err)
    }

    async fn remove(&self, id: &str, force: bool) -> Result<(), ContainerError> {
        use perry_container_compose::backend::ContainerBackend as CCB;
        CCB::remove(&self.inner, id, force).await.map_err(map_compose_err)
    }

    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>, ContainerError> {
        use perry_container_compose::backend::ContainerBackend as CCB;
        let infos = CCB::list(&self.inner, all).await.map_err(map_compose_err)?;
        Ok(infos.into_iter().map(compose_info_to_stdlib).collect())
    }

    async fn inspect(&self, id: &str) -> Result<ContainerInfo, ContainerError> {
        use perry_container_compose::backend::ContainerBackend as CCB;
        let info = CCB::inspect(&self.inner, id).await.map_err(map_compose_err)?;
        Ok(compose_info_to_stdlib(info))
    }

    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs, ContainerError> {
        use perry_container_compose::backend::ContainerBackend as CCB;
        let logs = CCB::logs(&self.inner, id, tail).await.map_err(map_compose_err)?;
        Ok(ContainerLogs {
            stdout: logs.stdout,
            stderr: logs.stderr,
        })
    }

    async fn exec(
        &self,
        id: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Result<ContainerLogs, ContainerError> {
        use perry_container_compose::backend::ContainerBackend as CCB;
        let logs = CCB::exec(&self.inner, id, cmd, env, workdir)
            .await
            .map_err(map_compose_err)?;
        Ok(ContainerLogs {
            stdout: logs.stdout,
            stderr: logs.stderr,
        })
    }

    async fn pull_image(&self, reference: &str) -> Result<(), ContainerError> {
        use perry_container_compose::backend::ContainerBackend as CCB;
        CCB::pull_image(&self.inner, reference).await.map_err(map_compose_err)
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>, ContainerError> {
        use perry_container_compose::backend::ContainerBackend as CCB;
        let images = CCB::list_images(&self.inner).await.map_err(map_compose_err)?;
        Ok(images.into_iter().map(|img| ImageInfo {
            id: img.id,
            repository: img.repository,
            tag: img.tag,
            size: img.size,
            created: img.created,
        }).collect())
    }

    async fn remove_image(&self, reference: &str, force: bool) -> Result<(), ContainerError> {
        use perry_container_compose::backend::ContainerBackend as CCB;
        CCB::remove_image(&self.inner, reference, force).await.map_err(map_compose_err)
    }

    async fn create_network(
        &self,
        name: &str,
        config: &ComposeNetwork,
    ) -> Result<(), ContainerError> {
        use perry_container_compose::backend::ContainerBackend as CCB;
        use perry_container_compose::backend::Backend as LegacyBackend;

        // Build a compose-crate ComposeNetwork from stdlib fields.
        // We use the legacy Backend trait's create_network which takes (name, driver, labels)
        // to avoid depending on indexmap in the stdlib.
        let labels_map: Option<HashMap<String, String>> = config
            .labels
            .as_ref()
            .map(|l| l.to_map())
            .filter(|m| !m.is_empty());
        LegacyBackend::create_network(
            &self.inner,
            name,
            config.driver.as_deref(),
            labels_map.as_ref(),
        )
        .await
        .map_err(map_compose_err)
    }

    async fn remove_network(&self, name: &str) -> Result<(), ContainerError> {
        use perry_container_compose::backend::ContainerBackend as CCB;
        CCB::remove_network(&self.inner, name).await.map_err(map_compose_err)
    }

    async fn create_volume(
        &self,
        name: &str,
        config: &ComposeVolume,
    ) -> Result<(), ContainerError> {
        use perry_container_compose::backend::Backend as LegacyBackend;

        let labels_map: Option<HashMap<String, String>> = config
            .labels
            .as_ref()
            .map(|l| l.to_map())
            .filter(|m| !m.is_empty());
        LegacyBackend::create_volume(
            &self.inner,
            name,
            config.driver.as_deref(),
            labels_map.as_ref(),
        )
        .await
        .map_err(map_compose_err)
    }

    async fn remove_volume(&self, name: &str) -> Result<(), ContainerError> {
        use perry_container_compose::backend::ContainerBackend as CCB;
        CCB::remove_volume(&self.inner, name).await.map_err(map_compose_err)
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

    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<(), ContainerError> {
        let binary = Self::find_binary().ok_or_else(|| ContainerError::BackendError {
            code: 1,
            message: "podman binary not found".to_string(),
        })?;
        let mut cmd = Command::new(&binary);
        cmd.arg("stop");
        if let Some(t) = timeout {
            cmd.arg(format!("--time={}", t));
        }
        cmd.arg(id);
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
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Result<ContainerLogs, ContainerError> {
        let binary = Self::find_binary().ok_or_else(|| ContainerError::BackendError {
            code: 1,
            message: "podman binary not found".to_string(),
        })?;
        let mut command = Command::new(&binary);
        command.arg("exec");
        if let Some(wd) = workdir {
            command.arg("--workdir").arg(wd);
        }
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
        config: &ComposeNetwork,
    ) -> Result<(), ContainerError> {
        let binary = Self::find_binary().ok_or_else(|| ContainerError::BackendError {
            code: 1,
            message: "podman binary not found".to_string(),
        })?;
        let mut cmd = Command::new(&binary);
        cmd.args(["network", "create"]);
        if let Some(d) = &config.driver {
            cmd.arg("--driver").arg(d);
        }
        if let Some(labels) = &config.labels {
            if let super::types::ListOrDict::Dict(map) = labels {
                for (k, v) in map {
                    if let Some(val) = v {
                        let val_str = match val {
                            serde_yaml::Value::String(s) => s.clone(),
                            serde_yaml::Value::Number(n) => n.to_string(),
                            serde_yaml::Value::Bool(b) => b.to_string(),
                            _ => "unknown".to_string(),
                        };
                        cmd.arg("--label").arg(format!("{}={}", k, val_str));
                    }
                }
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
        config: &ComposeVolume,
    ) -> Result<(), ContainerError> {
        let binary = Self::find_binary().ok_or_else(|| ContainerError::BackendError {
            code: 1,
            message: "podman binary not found".to_string(),
        })?;
        let mut cmd = Command::new(&binary);
        cmd.args(["volume", "create"]);
        if let Some(d) = &config.driver {
            cmd.arg("--driver").arg(d);
        }
        if let Some(labels) = &config.labels {
            if let super::types::ListOrDict::Dict(map) = labels {
                for (k, v) in map {
                    if let Some(val) = v {
                        let val_str = match val {
                            serde_yaml::Value::String(s) => s.clone(),
                            serde_yaml::Value::Number(n) => n.to_string(),
                            serde_yaml::Value::Bool(b) => b.to_string(),
                            _ => "unknown".to_string(),
                        };
                        cmd.arg("--label").arg(format!("{}={}", k, val_str));
                    }
                }
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

// ─── Backend Adapter ─────────────────────────────────────────────────────────

/// Bridges stdlib's `ContainerBackend` with compose crate's `ContainerBackend` trait.
pub struct BackendAdapter {
    pub inner: Arc<dyn ContainerBackend>,
}

#[async_trait]
impl perry_container_compose::backend::ContainerBackend for BackendAdapter {
    fn backend_name(&self) -> &str {
        self.inner.name()
    }

    async fn check_available(&self) -> perry_container_compose::Result<()> {
        self.inner.check_available().await.map_err(to_compose_err)
    }

    async fn run(&self, spec: &perry_container_compose::types::ContainerSpec) -> perry_container_compose::Result<perry_container_compose::types::ContainerHandle> {
        let s = stdlib_spec_from_compose(spec);
        self.inner.run(&s).await.map(|h| perry_container_compose::types::ContainerHandle {
            id: h.id,
            name: h.name,
        }).map_err(to_compose_err)
    }

    async fn create(&self, spec: &perry_container_compose::types::ContainerSpec) -> perry_container_compose::Result<perry_container_compose::types::ContainerHandle> {
        let s = stdlib_spec_from_compose(spec);
        self.inner.create(&s).await.map(|h| perry_container_compose::types::ContainerHandle {
            id: h.id,
            name: h.name,
        }).map_err(to_compose_err)
    }

    async fn start(&self, id: &str) -> perry_container_compose::Result<()> {
        self.inner.start(id).await.map_err(to_compose_err)
    }

    async fn stop(&self, id: &str, timeout: Option<u32>) -> perry_container_compose::Result<()> {
        self.inner.stop(id, timeout).await.map_err(to_compose_err)
    }

    async fn remove(&self, id: &str, force: bool) -> perry_container_compose::Result<()> {
        self.inner.remove(id, force).await.map_err(to_compose_err)
    }

    async fn list(&self, all: bool) -> perry_container_compose::Result<Vec<perry_container_compose::types::ContainerInfo>> {
        let list = self.inner.list(all).await.map_err(to_compose_err)?;
        Ok(list.into_iter().map(|i| perry_container_compose::types::ContainerInfo {
            id: i.id,
            name: i.name,
            image: i.image,
            status: i.status,
            ports: i.ports,
            created: i.created,
        }).collect())
    }

    async fn inspect(&self, id: &str) -> perry_container_compose::Result<perry_container_compose::types::ContainerInfo> {
        let info = self.inner.inspect(id).await.map_err(to_compose_err)?;
        Ok(perry_container_compose::types::ContainerInfo {
            id: info.id,
            name: info.name,
            image: info.image,
            status: info.status,
            ports: info.ports,
            created: info.created,
        })
    }

    async fn logs(&self, id: &str, tail: Option<u32>) -> perry_container_compose::Result<perry_container_compose::types::ContainerLogs> {
        let logs = self.inner.logs(id, tail).await.map_err(to_compose_err)?;
        Ok(perry_container_compose::types::ContainerLogs {
            stdout: logs.stdout,
            stderr: logs.stderr,
        })
    }

    async fn exec(
        &self,
        id: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> perry_container_compose::Result<perry_container_compose::types::ContainerLogs> {
        let logs = self.inner.exec(id, cmd, env, workdir).await.map_err(to_compose_err)?;
        Ok(perry_container_compose::types::ContainerLogs {
            stdout: logs.stdout,
            stderr: logs.stderr,
        })
    }

    async fn pull_image(&self, reference: &str) -> perry_container_compose::Result<()> {
        self.inner.pull_image(reference).await.map_err(to_compose_err)
    }

    async fn list_images(&self) -> perry_container_compose::Result<Vec<perry_container_compose::types::ImageInfo>> {
        let images = self.inner.list_images().await.map_err(to_compose_err)?;
        Ok(images.into_iter().map(|img| perry_container_compose::types::ImageInfo {
            id: img.id,
            repository: img.repository,
            tag: img.tag,
            size: img.size,
            created: img.created,
        }).collect())
    }

    async fn remove_image(&self, reference: &str, force: bool) -> perry_container_compose::Result<()> {
        self.inner.remove_image(reference, force).await.map_err(to_compose_err)
    }

    async fn create_network(&self, name: &str, config: &perry_container_compose::backend::NetworkConfig) -> perry_container_compose::Result<()> {
        let c = perry_container_compose::types::ComposeNetwork {
            driver: config.driver.clone(),
            labels: if config.labels.is_empty() { None } else { Some(perry_container_compose::types::ListOrDict::Dict(config.labels.iter().map(|(k, v)| (k.clone(), Some(serde_yaml::Value::String(v.clone())))).collect())) },
            internal: Some(config.internal),
            enable_ipv6: Some(config.enable_ipv6),
            ..Default::default()
        };
        self.inner.create_network(name, &c).await.map_err(to_compose_err)
    }

    async fn remove_network(&self, name: &str) -> perry_container_compose::Result<()> {
        self.inner.remove_network(name).await.map_err(to_compose_err)
    }

    async fn create_volume(&self, name: &str, config: &perry_container_compose::backend::VolumeConfig) -> perry_container_compose::Result<()> {
        let c = perry_container_compose::types::ComposeVolume {
            driver: config.driver.clone(),
            labels: if config.labels.is_empty() { None } else { Some(perry_container_compose::types::ListOrDict::Dict(config.labels.iter().map(|(k, v)| (k.clone(), Some(serde_yaml::Value::String(v.clone())))).collect())) },
            ..Default::default()
        };
        self.inner.create_volume(name, &c).await.map_err(to_compose_err)
    }

    async fn remove_volume(&self, name: &str) -> perry_container_compose::Result<()> {
        self.inner.remove_volume(name).await.map_err(to_compose_err)
    }
}

fn stdlib_spec_from_compose(s: &perry_container_compose::types::ContainerSpec) -> ContainerSpec {
    ContainerSpec {
        image: s.image.clone(),
        name: s.name.clone(),
        ports: s.ports.clone(),
        volumes: s.volumes.clone(),
        env: s.env.clone(),
        cmd: s.cmd.clone(),
        entrypoint: s.entrypoint.clone(),
        network: s.network.clone(),
        rm: s.rm,
    }
}

fn to_compose_err(e: ContainerError) -> perry_container_compose::error::ComposeError {
    match e {
        ContainerError::NotFound(id) => perry_container_compose::error::ComposeError::NotFound(id),
        ContainerError::DependencyCycle { cycle } => perry_container_compose::error::ComposeError::DependencyCycle { services: cycle },
        ContainerError::ServiceStartupFailed { service, error } => perry_container_compose::error::ComposeError::ServiceStartupFailed { service, message: error },
        other => perry_container_compose::error::ComposeError::BackendError {
            code: 1,
            message: other.to_string(),
        },
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

pub fn get_global_backend() -> Arc<dyn ContainerBackend> {
    GLOBAL_BACKEND.get_or_init(|| {
        get_backend().expect("Failed to initialize container backend")
    }).clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_backend_non_null() {
        let backend = get_backend();
        assert!(backend.is_ok());
        let b = backend.unwrap();
        #[cfg(target_os = "macos")]
        assert_eq!(b.name(), "apple/container");
        #[cfg(not(target_os = "macos"))]
        assert_eq!(b.name(), "podman");
    }
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

#[cfg(target_os = "macos")]
fn compose_info_to_stdlib(
    info: perry_container_compose::types::ContainerInfo,
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
