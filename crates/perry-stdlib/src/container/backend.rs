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
use std::sync::Arc;
use tokio::process::Command;

pub use perry_container_compose::backend::{
    ContainerBackend as ComposeBackend, CliBackend, CliProtocol, DockerProtocol, AppleContainerProtocol,
    LimaProtocol, detect_backend,
};

// ─── ContainerBackend trait ───────────────────────────────────────────────────
//
// Mirrors perry_container_compose::backend::ContainerBackend but uses the
// stdlib's own type aliases (serde_json-based) so the rest of the stdlib
// does not need to depend on serde_yaml.

#[async_trait]
pub trait ContainerBackend: Send + Sync {
    /// Backend name for display (e.g. "apple-container", "podman")
    fn backend_name(&self) -> &str;

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

// ─── Backend Adapter ─────────────────────────────────────────────────────────

/// Bridges stdlib's `ContainerBackend` with perry-container-compose's `ContainerBackend` trait.
pub struct BackendAdapter {
    pub inner: Arc<dyn perry_container_compose::backend::ContainerBackend>,
}

impl From<Arc<dyn perry_container_compose::backend::ContainerBackend>> for BackendAdapter {
    fn from(inner: Arc<dyn perry_container_compose::backend::ContainerBackend>) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl ContainerBackend for BackendAdapter {
    fn backend_name(&self) -> &str {
        self.inner.backend_name()
    }

    async fn check_available(&self) -> Result<(), ContainerError> {
        self.inner.check_available().await.map_err(ContainerError::from)
    }

    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle, ContainerError> {
        let cspec = perry_container_compose::types::ContainerSpec {
            image: spec.image.clone(),
            name: spec.name.clone(),
            ports: spec.ports.clone(),
            volumes: spec.volumes.clone(),
            env: spec.env.clone(),
            cmd: spec.cmd.clone(),
            entrypoint: spec.entrypoint.clone(),
            network: spec.network.clone(),
            rm: spec.rm,
        };
        let h = self.inner.run(&cspec).await.map_err(ContainerError::from)?;
        Ok(ContainerHandle { id: h.id, name: h.name })
    }

    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle, ContainerError> {
        let cspec = perry_container_compose::types::ContainerSpec {
            image: spec.image.clone(),
            name: spec.name.clone(),
            ports: spec.ports.clone(),
            volumes: spec.volumes.clone(),
            env: spec.env.clone(),
            cmd: spec.cmd.clone(),
            entrypoint: spec.entrypoint.clone(),
            network: spec.network.clone(),
            rm: spec.rm,
        };
        let h = self.inner.create(&cspec).await.map_err(ContainerError::from)?;
        Ok(ContainerHandle { id: h.id, name: h.name })
    }

    async fn start(&self, id: &str) -> Result<(), ContainerError> {
        self.inner.start(id).await.map_err(ContainerError::from)
    }

    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<(), ContainerError> {
        self.inner.stop(id, timeout).await.map_err(ContainerError::from)
    }

    async fn remove(&self, id: &str, force: bool) -> Result<(), ContainerError> {
        self.inner.remove(id, force).await.map_err(ContainerError::from)
    }

    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>, ContainerError> {
        let infos = self.inner.list(all).await.map_err(ContainerError::from)?;
        Ok(infos.into_iter().map(|i| ContainerInfo {
            id: i.id,
            name: i.name,
            image: i.image,
            status: i.status,
            ports: i.ports,
            created: i.created,
        }).collect())
    }

    async fn inspect(&self, id: &str) -> Result<ContainerInfo, ContainerError> {
        let i = self.inner.inspect(id).await.map_err(ContainerError::from)?;
        Ok(ContainerInfo {
            id: i.id,
            name: i.name,
            image: i.image,
            status: i.status,
            ports: i.ports,
            created: i.created,
        })
    }

    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs, ContainerError> {
        let logs = self.inner.logs(id, tail).await.map_err(ContainerError::from)?;
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
        let logs = self.inner.exec(id, cmd, env, workdir)
            .await
            .map_err(ContainerError::from)?;
        Ok(ContainerLogs {
            stdout: logs.stdout,
            stderr: logs.stderr,
        })
    }

    async fn pull_image(&self, reference: &str) -> Result<(), ContainerError> {
        self.inner.pull_image(reference).await.map_err(ContainerError::from)
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>, ContainerError> {
        let images = self.inner.list_images().await.map_err(ContainerError::from)?;
        Ok(images.into_iter().map(|img| ImageInfo {
            id: img.id,
            repository: img.repository,
            tag: img.tag,
            size: img.size,
            created: img.created,
        }).collect())
    }

    async fn remove_image(&self, reference: &str, force: bool) -> Result<(), ContainerError> {
        self.inner.remove_image(reference, force).await.map_err(ContainerError::from)
    }

    async fn create_network(
        &self,
        name: &str,
        config: &ComposeNetwork,
    ) -> Result<(), ContainerError> {
        self.inner.create_network(name, config).await.map_err(ContainerError::from)
    }

    async fn remove_network(&self, name: &str) -> Result<(), ContainerError> {
        self.inner.remove_network(name).await.map_err(ContainerError::from)
    }

    async fn create_volume(
        &self,
        name: &str,
        config: &ComposeVolume,
    ) -> Result<(), ContainerError> {
        self.inner.create_volume(name, config).await.map_err(ContainerError::from)
    }

    async fn remove_volume(&self, name: &str) -> Result<(), ContainerError> {
        self.inner.remove_volume(name).await.map_err(ContainerError::from)
    }
}

// ─── Backend selection ────────────────────────────────────────────────────────

pub fn get_backend() -> Result<Arc<dyn ContainerBackend>, ContainerError> {
    // If PERRY_CONTAINER_BACKEND is set, detect_backend will use it.
    // get_global_backend_instance in mod.rs now calls detect_backend directly.
    // This function is kept for legacy compatibility but redirected.
    let backend = tokio::runtime::Handle::current().block_on(perry_container_compose::backend::detect_backend())
        .map_err(|probed| ContainerError::NoBackendFound {
            probed
        })?;
    Ok(Arc::new(BackendAdapter { inner: Arc::new(backend) }))
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
