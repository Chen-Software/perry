//! ComposeWrapper — thin orchestration adapter over `perry_container_compose::ComposeEngine`.

use perry_container_compose::backend::ContainerBackend;
use super::types::{
    ComposeHandle, ComposeSpec, ContainerError, ContainerInfo, ContainerLogs,
};
use std::sync::Arc;
use perry_container_compose::ComposeEngine;
use std::collections::HashMap;

pub struct ComposeWrapper {
    engine: Arc<ComposeEngine>,
}

impl ComposeWrapper {
    /// Create a new `ComposeWrapper` from a spec and backend.
    pub fn new(spec: perry_container_compose::ComposeSpec, project_name: String, backend: Arc<dyn ContainerBackend>) -> Self {
        Self {
            engine: Arc::new(ComposeEngine::new(spec, project_name, backend)),
        }
    }

    /// Create a `ComposeWrapper` from an already-existing engine (e.g. looked up by stack ID).
    pub fn from_engine(engine: Arc<ComposeEngine>) -> Self {
        Self { engine }
    }

    pub async fn up(&self) -> Result<perry_container_compose::types::ComposeHandle, ContainerError> {
        self.engine
            .up(&[], true, true, false)
            .await
            .map_err(|e| ContainerError::BackendError { code: 500, message: e.to_string() })
    }

    pub async fn down(&self, volumes: bool) -> Result<(), ContainerError> {
        self.engine
            .down(&[], false, volumes)
            .await
            .map_err(|e| ContainerError::BackendError { code: 500, message: e.to_string() })
    }

    pub async fn ps(&self) -> Result<Vec<perry_container_compose::types::ContainerInfo>, ContainerError> {
        self.engine
            .ps()
            .await
            .map_err(|e| ContainerError::BackendError { code: 500, message: e.to_string() })
    }

    pub async fn logs(
        &self,
        services: &[String],
        tail: Option<u32>,
    ) -> Result<ContainerLogs, ContainerError> {
        let logs_map = self.engine.logs(services, tail).await.map_err(|e| ContainerError::BackendError { code: 500, message: e.to_string() })?;
        let mut stdout = String::new();
        let mut stderr = String::new();
        for logs in logs_map.values() {
            stdout.push_str(&logs.stdout);
            stdout.push('\n');
            stderr.push_str(&logs.stderr);
            stderr.push('\n');
        }
        Ok(ContainerLogs {
            stdout,
            stderr,
        })
    }

    pub async fn exec(
        &self,
        service: &str,
        cmd: &[String],
    ) -> Result<perry_container_compose::types::ContainerLogs, ContainerError> {
        self.engine
            .exec(service, cmd, None, None)
            .await
            .map_err(|e| ContainerError::BackendError { code: 500, message: e.to_string() })
    }

    pub async fn start(&self, services: &[String]) -> Result<(), ContainerError> {
        self.engine.start(services).await.map_err(|e| ContainerError::BackendError { code: 500, message: e.to_string() })
    }

    pub async fn stop(&self, services: &[String]) -> Result<(), ContainerError> {
        self.engine.stop(services).await.map_err(|e| ContainerError::BackendError { code: 500, message: e.to_string() })
    }

    pub async fn restart(&self, services: &[String]) -> Result<(), ContainerError> {
        self.engine.restart(services).await.map_err(|e| ContainerError::BackendError { code: 500, message: e.to_string() })
    }
}

/// Create a new compose stack from a spec and backend, bring it up, and return the handle.
pub async fn compose_up(
    spec: perry_container_compose::ComposeSpec,
    backend: Arc<dyn ContainerBackend>,
) -> Result<perry_container_compose::types::ComposeHandle, ContainerError> {
    let project_name = spec.name.clone().unwrap_or_else(|| "perry-stack".to_string());
    let wrapper = ComposeWrapper::new(spec, project_name, backend);
    wrapper.up().await
}
