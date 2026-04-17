//! ComposeWrapper — thin orchestration adapter over `perry_container_compose::ComposeEngine`.

use perry_container_compose::backend::ContainerBackend;
use super::types::{
    ComposeSpec, ContainerError, ContainerLogs,
};
use std::sync::Arc;
use perry_container_compose::ComposeEngine;

pub struct ComposeWrapper {
    engine: Arc<ComposeEngine>,
}

impl ComposeWrapper {
    /// Create a new `ComposeWrapper` from a spec and backend.
    pub fn new(spec: perry_container_compose::ComposeSpec, backend: Arc<dyn ContainerBackend>) -> Self {
        let project_name = spec.name.clone().unwrap_or_else(|| "perry-stack".to_string());
        Self {
            engine: Arc::new(ComposeEngine::new(spec, project_name, backend)),
        }
    }

    /// Create a `ComposeWrapper` from an already-existing engine.
    pub fn from_engine(engine: Arc<ComposeEngine>) -> Self {
        Self { engine }
    }

    pub async fn up(&self) -> Result<perry_container_compose::types::ComposeHandle, ContainerError> {
        self.engine.up(&[], true, true, false).await.map_err(map_to_stdlib_err)
    }

    pub async fn down(&self, volumes: bool) -> Result<(), ContainerError> {
        self.engine.down(&[], false, volumes).await.map_err(map_to_stdlib_err)
    }

    pub async fn ps(&self) -> Result<Vec<perry_container_compose::types::ContainerInfo>, ContainerError> {
        self.engine.ps().await.map_err(map_to_stdlib_err)
    }

    pub async fn logs(&self, services: &[String], tail: Option<u32>) -> Result<std::collections::HashMap<String, String>, ContainerError> {
        self.engine.logs(services, tail).await.map_err(map_to_stdlib_err)
    }

    pub async fn exec(&self, service: &str, cmd: &[String], env: Option<&std::collections::HashMap<String, String>>, workdir: Option<&str>) -> Result<perry_container_compose::types::ContainerLogs, ContainerError> {
        self.engine.exec(service, cmd, env, workdir).await.map_err(map_to_stdlib_err)
    }

    pub async fn start(&self, services: &[String]) -> Result<(), ContainerError> {
        self.engine.start(services).await.map_err(map_to_stdlib_err)
    }

    pub async fn stop(&self, services: &[String]) -> Result<(), ContainerError> {
        self.engine.stop(services).await.map_err(map_to_stdlib_err)
    }

    pub async fn restart(&self, services: &[String]) -> Result<(), ContainerError> {
        self.engine.restart(services).await.map_err(map_to_stdlib_err)
    }
}

fn map_to_stdlib_err(e: perry_container_compose::error::ComposeError) -> ContainerError {
    match e {
        perry_container_compose::error::ComposeError::NotFound(id) => ContainerError::NotFound(id),
        perry_container_compose::error::ComposeError::DependencyCycle { services } => ContainerError::DependencyCycle { cycle: services },
        perry_container_compose::error::ComposeError::ServiceStartupFailed { service, message } => ContainerError::ServiceStartupFailed { service, error: message },
        other => ContainerError::BackendError { code: -1, message: other.to_string() },
    }
}

/// Create a new compose stack from a spec and backend, bring it up, and return the handle.
pub async fn compose_up(
    spec: perry_container_compose::ComposeSpec,
    backend: Arc<dyn ContainerBackend>,
) -> Result<perry_container_compose::types::ComposeHandle, ContainerError> {
    let wrapper = ComposeWrapper::new(spec, backend);
    wrapper.up().await
}
