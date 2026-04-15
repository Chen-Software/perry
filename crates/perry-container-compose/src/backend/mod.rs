//! Backend implementations for container operations.
//!
//! Currently supports Apple Container (macOS/iOS) as the primary backend.
//! Future: Podman backend for Linux and other platforms.

pub mod apple;

pub use apple::AppleContainerBackend;

use crate::commands::ContainerStatus;
use crate::error::Result;
use async_trait::async_trait;
use std::collections::HashMap;

/// Information about a running (or stopped) container
#[derive(Debug, Clone)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub ports: Vec<String>,
    pub created: String,
}

/// Result of an exec call
#[derive(Debug, Clone)]
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Abstraction over different container backends
#[async_trait]
pub trait Backend: Send + Sync {
    /// Backend name for display purposes
    fn name(&self) -> &'static str;

    /// Build an image
    async fn build(
        &self,
        context: &str,
        dockerfile: Option<&str>,
        tag: &str,
        args: Option<&HashMap<String, String>>,
        target: Option<&str>,
        network: Option<&str>,
    ) -> Result<()>;

    /// Run a container (create + start)
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

    /// Start an existing stopped container
    async fn start(&self, name: &str) -> Result<()>;

    /// Stop a running container
    async fn stop(&self, name: &str) -> Result<()>;

    /// Remove a container
    async fn remove(&self, name: &str, force: bool) -> Result<()>;

    /// Inspect a container and return its status
    async fn inspect(&self, name: &str) -> Result<ContainerStatus>;

    /// List all containers matching a label
    async fn list(&self, label_filter: Option<&str>) -> Result<Vec<ContainerInfo>>;

    /// Fetch logs from a container
    async fn logs(&self, name: &str, tail: Option<u32>, follow: bool) -> Result<String>;

    /// Execute a command inside a running container
    async fn exec(
        &self,
        name: &str,
        cmd: &[String],
        user: Option<&str>,
        workdir: Option<&str>,
        env: Option<&HashMap<String, String>>,
    ) -> Result<ExecResult>;
}

/// Select the best available backend for the current platform.
///
/// macOS/iOS  → AppleContainerBackend  
/// Other      → (future) PodmanBackend  
pub fn get_backend() -> Result<Box<dyn Backend>> {
    #[cfg(target_os = "macos")]
    {
        return Ok(Box::new(AppleContainerBackend::new()));
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err(crate::error::BackendError::NotAvailable {
            reason: "Only macOS (Apple Container) is supported at this time".to_string(),
        }
        .into())
    }
}
