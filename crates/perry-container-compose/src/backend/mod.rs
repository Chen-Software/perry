//! Container backend abstraction.
//!
//! Defines the `ContainerBackend` async trait, platform-specific
//! implementations (Apple Container on macOS, Podman elsewhere), and
//! the `get_backend()` platform selector.

pub mod apple;
#[cfg(not(target_os = "macos"))]
pub mod podman;

pub use apple::AppleContainerBackend;
#[cfg(not(target_os = "macos"))]
pub use podman::PodmanBackend;

use crate::error::Result;
use crate::types::{
    ComposeNetwork, ComposeVolume, ContainerHandle, ContainerInfo,
    ContainerLogs, ContainerSpec, ImageInfo,
};
use async_trait::async_trait;
use std::collections::HashMap;

/// Abstraction over different container backends.
///
/// All async methods correspond to single CLI invocations under the hood.
#[async_trait]
pub trait ContainerBackend: Send + Sync {
    /// Backend name for display (e.g. "apple-container", "podman")
    fn name(&self) -> &'static str;

    /// Check whether the backend binary is available on PATH.
    async fn check_available(&self) -> Result<()>;

    /// Run a container (create + start). Returns a handle.
    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle>;

    /// Create a container (without starting it).
    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle>;

    /// Start an existing stopped container.
    async fn start(&self, id: &str) -> Result<()>;

    /// Stop a running container.
    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()>;

    /// Remove a container.
    async fn remove(&self, id: &str, force: bool) -> Result<()>;

    /// List all containers.
    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>>;

    /// Inspect a container.
    async fn inspect(&self, id: &str) -> Result<ContainerInfo>;

    /// Fetch logs from a container.
    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs>;

    /// Execute a command inside a running container.
    async fn exec(
        &self,
        id: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Result<ContainerLogs>;

    /// Pull an image.
    async fn pull_image(&self, reference: &str) -> Result<()>;

    /// List images.
    async fn list_images(&self) -> Result<Vec<ImageInfo>>;

    /// Remove an image.
    async fn remove_image(&self, reference: &str, force: bool) -> Result<()>;

    /// Create a network.
    async fn create_network(&self, name: &str, config: &ComposeNetwork) -> Result<()>;

    /// Remove a network (idempotent).
    async fn remove_network(&self, name: &str) -> Result<()>;

    /// Create a volume.
    async fn create_volume(&self, name: &str, config: &ComposeVolume) -> Result<()>;

    /// Remove a volume (idempotent).
    async fn remove_volume(&self, name: &str) -> Result<()>;
}

// ============ Legacy Backend trait (for backward compat with Orchestrator) ============

/// Result of inspecting a container status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContainerStatus {
    Running,
    Stopped,
    NotFound,
}

impl ContainerStatus {
    pub fn is_running(&self) -> bool {
        matches!(self, ContainerStatus::Running)
    }

    pub fn exists(&self) -> bool {
        !matches!(self, ContainerStatus::NotFound)
    }
}

/// Result of running exec inside a container
#[derive(Debug, Clone)]
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Legacy backend trait used by the Orchestrator (wraps ContainerBackend).
/// Kept for backward compatibility with the CLI path.
#[async_trait]
pub trait Backend: Send + Sync {
    fn name(&self) -> &'static str;

    async fn build(
        &self,
        context: &str,
        dockerfile: Option<&str>,
        tag: &str,
        args: Option<&HashMap<String, String>>,
        target: Option<&str>,
        network: Option<&str>,
    ) -> Result<()>;

    async fn run(
        &self,
        image: &str,
        name: &str,
        ports: Option<&[String]>,
        env: Option<&HashMap<String, String>>,
        volumes: Option<&[String]>,
        labels: Option<&HashMap<String, String>>,
        cmd: Option<&[String]>,
        detach: bool,
    ) -> Result<()>;

    async fn start(&self, name: &str) -> Result<()>;
    async fn stop(&self, name: &str) -> Result<()>;
    async fn remove(&self, name: &str, force: bool) -> Result<()>;
    async fn inspect(&self, name: &str) -> Result<ContainerStatus>;
    async fn list(&self, label_filter: Option<&str>) -> Result<Vec<ContainerInfo>>;
    async fn logs(&self, name: &str, tail: Option<u32>, follow: bool) -> Result<String>;
    async fn exec(
        &self,
        name: &str,
        cmd: &[String],
        user: Option<&str>,
        workdir: Option<&str>,
        env: Option<&HashMap<String, String>>,
    ) -> Result<ExecResult>;

    async fn create_network(
        &self,
        name: &str,
        driver: Option<&str>,
        labels: Option<&HashMap<String, String>>,
    ) -> Result<()>;

    async fn remove_network(&self, name: &str) -> Result<()>;

    async fn create_volume(
        &self,
        name: &str,
        driver: Option<&str>,
        labels: Option<&HashMap<String, String>>,
    ) -> Result<()>;

    async fn remove_volume(&self, name: &str) -> Result<()>;
}

/// Select the best available backend for the current platform.
///
/// macOS/iOS → AppleContainerBackend
/// Other     → PodmanBackend (if available)
pub fn get_backend() -> Result<Box<dyn Backend>> {
    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(AppleContainerBackend::new()))
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(Box::new(PodmanBackend::new()))
    }
}

/// Get a `ContainerBackend` (new API) for the current platform.
pub fn get_container_backend() -> Result<Box<dyn ContainerBackend>> {
    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(AppleContainerBackend::new()))
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(Box::new(PodmanBackend::new()))
    }
}
