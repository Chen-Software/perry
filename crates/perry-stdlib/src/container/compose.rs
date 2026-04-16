//! ComposeWrapper — thin orchestration adapter over `perry_container_compose::ComposeEngine`.

use perry_container_compose::backend::ContainerBackend;
use perry_container_compose::types::{
    ComposeHandle, ComposeSpec, ContainerInfo, ContainerLogs,
};
use crate::container::types::ContainerError;
use std::sync::Arc;
use perry_container_compose::ComposeEngine;

pub struct ComposeWrapper {
    pub engine: Arc<ComposeEngine>,
}

impl ComposeWrapper {
    /// Create a new `ComposeWrapper` from a spec and backend.
    pub fn new(spec: ComposeSpec, backend: Arc<dyn ContainerBackend>) -> Self {
        let project_name = spec.name.clone().unwrap_or_else(|| "perry-stack".to_string());
        Self {
            engine: Arc::new(ComposeEngine::new(spec, project_name, backend)),
        }
    }

    pub async fn up(&self) -> Result<ComposeHandle, ContainerError> {
        self.engine.up(&[], true, false, false).await.map_err(|e| ContainerError::ServiceStartupFailed { service: "compose".to_string(), error: e.to_string() })
    }

    pub async fn down(&self, volumes: bool) -> Result<(), ContainerError> {
        self.engine.down(&[], false, volumes).await.map_err(|e| ContainerError::BackendError { code: 1, message: e.to_string() })
    }

    pub async fn ps(&self) -> Result<Vec<ContainerInfo>, ContainerError> {
        self.engine.ps().await.map_err(|e| ContainerError::BackendError { code: 1, message: e.to_string() })
    }

    pub async fn logs(&self, services: &[String], tail: Option<u32>) -> Result<String, ContainerError> {
        let logs = self.engine.logs(services, tail).await.map_err(|e| ContainerError::BackendError { code: 1, message: e.to_string() })?;
        Ok(logs.values().cloned().collect::<Vec<_>>().join("\n"))
    }

    pub async fn exec(&self, service: &str, cmd: &[String]) -> Result<ContainerLogs, ContainerError> {
        self.engine.exec(service, cmd, None, None).await.map_err(|e| ContainerError::BackendError { code: 1, message: e.to_string() })
    }

    pub async fn start(&self, services: &[String]) -> Result<(), ContainerError> {
        self.engine.start(services).await.map_err(|e| ContainerError::BackendError { code: 1, message: e.to_string() })
    }

    pub async fn stop(&self, services: &[String]) -> Result<(), ContainerError> {
        self.engine.stop(services).await.map_err(|e| ContainerError::BackendError { code: 1, message: e.to_string() })
    }

    pub async fn restart(&self, services: &[String]) -> Result<(), ContainerError> {
        self.engine.restart(services).await.map_err(|e| ContainerError::BackendError { code: 1, message: e.to_string() })
    }
}

/// Create a new compose stack from a spec and backend, bring it up, and return the handle.
pub async fn compose_up(
    spec: ComposeSpec,
    backend: Arc<dyn ContainerBackend>,
) -> Result<ComposeHandle, ContainerError> {
    let wrapper = ComposeWrapper::new(spec, backend);
    wrapper.up().await
}
