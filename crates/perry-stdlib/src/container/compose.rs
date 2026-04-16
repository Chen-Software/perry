//! ComposeWrapper — thin orchestration adapter over `perry_container_compose::ComposeEngine`.

use super::types::{
    ComposeHandle, ContainerError, ContainerInfo, ContainerLogs,
};
use std::sync::Arc;
use perry_container_compose::ComposeEngine;
use perry_container_compose::types::{ComposeSpec as RawComposeSpec, ComposeHandle as RawComposeHandle};
use perry_container_compose::backend::ContainerBackend as RawContainerBackend;

pub struct ComposeWrapper {
    engine: Arc<ComposeEngine>,
}

impl ComposeWrapper {
    /// Create a new `ComposeWrapper` from a spec and backend.
    pub fn new(spec: RawComposeSpec, backend: Arc<dyn RawContainerBackend>) -> Self {
        let project_name = spec.name.clone().unwrap_or_else(|| "perry-stack".to_string());
        Self {
            engine: Arc::new(ComposeEngine::new(spec, project_name, backend)),
        }
    }

    /// Create a `ComposeWrapper` from an already-existing engine (e.g. looked up by stack ID).
    pub fn from_engine(engine: Arc<ComposeEngine>) -> Self {
        Self { engine }
    }

    pub async fn up(&self) -> Result<RawComposeHandle, ContainerError> {
        self.engine.up(&[], true, true, false).await.map_err(ContainerError::from)
    }

    pub async fn down(&self, volumes: bool) -> Result<(), ContainerError> {
        self.engine.down(&[], false, volumes).await.map_err(ContainerError::from)
    }

    pub async fn ps(&self) -> Result<Vec<ContainerInfo>, ContainerError> {
        let infos = self.engine.ps().await.map_err(ContainerError::from)?;
        Ok(infos.into_iter().map(ContainerInfo::from).collect())
    }

    pub async fn logs(&self, service: Option<&str>, tail: Option<u32>) -> Result<ContainerLogs, ContainerError> {
        let services = service.map(|s| vec![s.to_string()]).unwrap_or_default();
        let logs_map = self.engine.logs(&services, tail).await.map_err(ContainerError::from)?;
        let combined = logs_map.values().cloned().collect::<Vec<_>>().join("\n");
        Ok(ContainerLogs {
            stdout: combined,
            stderr: String::new(),
        })
    }

    pub async fn exec(&self, service: &str, cmd: &[String]) -> Result<ContainerLogs, ContainerError> {
        let res = self.engine.exec(service, cmd, None, None).await.map_err(ContainerError::from)?;
        Ok(ContainerLogs {
            stdout: res.stdout,
            stderr: res.stderr,
        })
    }

    pub async fn start(&self, services: &[String]) -> Result<(), ContainerError> {
        self.engine.start(services).await.map_err(ContainerError::from)
    }

    pub async fn stop(&self, services: &[String]) -> Result<(), ContainerError> {
        self.engine.stop(services).await.map_err(ContainerError::from)
    }

    pub async fn restart(&self, services: &[String]) -> Result<(), ContainerError> {
        self.engine.restart(services).await.map_err(ContainerError::from)
    }
}

/// Create a new compose stack from a spec and backend, bring it up, and return the handle.
pub async fn compose_up(
    spec: RawComposeSpec,
    backend: Arc<dyn RawContainerBackend>,
) -> Result<RawComposeHandle, ContainerError> {
    let wrapper = ComposeWrapper::new(spec, backend);
    wrapper.up().await
}

/// Look up an existing engine by stack ID and wrap it in a `ComposeWrapper`.
/// Returns `None` if no engine with that stack ID is registered.
pub fn get_engine_wrapper(stack_id: u64) -> Option<ComposeWrapper> {
    ComposeEngine::get_engine(stack_id).map(ComposeWrapper::from_engine)
}
