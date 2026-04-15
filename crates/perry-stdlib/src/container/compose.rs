//! ComposeWrapper — thin orchestration adapter over `perry_container_compose::ComposeEngine`.

use perry_container_compose::backend::ContainerBackend;
use super::types::{
    ComposeHandle, ComposeSpec, ContainerError, ContainerInfo, ContainerLogs,
};
use std::sync::Arc;
use perry_container_compose::ComposeEngine;

pub struct ComposeWrapper {
    engine: Arc<ComposeEngine>,
}

impl ComposeWrapper {
    pub fn new(spec: ComposeSpec, backend: Arc<dyn ContainerBackend>) -> Self {
        let project_name = spec.name.clone().unwrap_or_else(|| "perry-stack".to_string());

        Self {
            engine: Arc::new(ComposeEngine::new(spec, project_name, backend)),
        }
    }

    pub async fn up(&self) -> Result<ComposeHandle, ContainerError> {
        self.engine.up(&[], true, false, false).await.map_err(Into::into)
    }

    pub async fn down(&self, _handle: &ComposeHandle, volumes: bool) -> Result<(), ContainerError> {
        self.engine.down(&[], false, volumes).await.map_err(Into::into)
    }

    pub async fn ps(&self, _handle: &ComposeHandle) -> Result<Vec<ContainerInfo>, ContainerError> {
        self.engine.ps().await.map_err(Into::into)
    }

    pub async fn logs(
        &self,
        _handle: &ComposeHandle,
        service: Option<&str>,
        tail: Option<u32>,
    ) -> Result<ContainerLogs, ContainerError> {
        let services: Vec<String> = service.map(|s| vec![s.to_string()]).unwrap_or_default();
        let map = self.engine.logs(&services, tail).await.map_err(ContainerError::from)?;
        let stdout = map.values().cloned().collect::<Vec<_>>().join("\n");
        Ok(ContainerLogs { stdout, stderr: String::new() })
    }

    pub async fn exec(
        &self,
        _handle: &ComposeHandle,
        service: &str,
        cmd: &[String],
    ) -> Result<ContainerLogs, ContainerError> {
        self.engine.exec(service, cmd, None, None).await.map_err(Into::into)
    }
}

/// Convenience function: create a `ComposeEngine` from `spec` and call `up()`.
///
/// Returns a `ComposeHandle` that can be used to manage the running stack.
pub async fn compose_up(
    spec: ComposeSpec,
    backend: Arc<dyn ContainerBackend>,
) -> Result<ComposeHandle, ContainerError> {
    ComposeWrapper::new(spec, backend).up().await
}
