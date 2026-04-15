//! Command trait interfaces — mirrors internal/commands/ in the original Go project.
//!
//! Each trait represents an operation that can be dispatched to a container backend.
//! Implementations live in `crate::backend`.

use crate::error::Result;
use async_trait::async_trait;

/// Result of inspecting a container
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContainerStatus {
    /// Container is running
    Running,
    /// Container exists but is not running
    Stopped,
    /// Container does not exist
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

/// Inspect a container and return its current status
#[async_trait]
pub trait InspectCommand: Send + Sync {
    async fn exec(&self) -> Result<ContainerStatus>;
}

/// Build a container image
#[async_trait]
pub trait BuildCommand: Send + Sync {
    async fn exec(&self) -> Result<()>;
    fn set_tag(&mut self, tag: String);
}

/// Run (create + start) a container
#[async_trait]
pub trait RunCommand: Send + Sync {
    async fn exec(&self) -> Result<()>;
    fn set_tag(&mut self, tag: String);
    fn set_name(&mut self, name: String);
}

/// Start an existing (stopped) container
#[async_trait]
pub trait StartCommand: Send + Sync {
    async fn exec(&self) -> Result<()>;
}

/// Stop a running container
#[async_trait]
pub trait StopCommand: Send + Sync {
    async fn exec(&self) -> Result<()>;
}

/// Remove a container
#[async_trait]
pub trait RemoveCommand: Send + Sync {
    async fn exec(&self) -> Result<()>;
}

/// Get logs from a container
#[async_trait]
pub trait LogsCommand: Send + Sync {
    async fn exec(&self, tail: Option<u32>, follow: bool) -> Result<String>;
}

/// Execute a command inside a container
#[async_trait]
pub trait ExecCommand: Send + Sync {
    async fn exec(
        &self,
        cmd: &[String],
        user: Option<&str>,
        workdir: Option<&str>,
        env: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<ExecResult>;
}

/// Result of running exec inside a container
#[derive(Debug, Clone)]
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}
