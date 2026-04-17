//! ComposeWrapper — thin orchestration adapter over `ContainerBackend`.

use super::backend::ContainerBackend;
use super::types::{
    ComposeHandle, ComposeSpec, ContainerError, ContainerLogs, ContainerInfo
};
use std::sync::Arc;

pub struct ComposeWrapper {
    spec: ComposeSpec,
    backend: Arc<dyn ContainerBackend>,
}

impl ComposeWrapper {
    pub fn new(spec: ComposeSpec, backend: Arc<dyn ContainerBackend>) -> Self {
        Self { spec, backend }
    }

    pub async fn up(&self) -> Result<ComposeHandle, ContainerError> {
        let mut engine = perry_container_compose::ComposeEngine::new(
            self.spec.clone(),
            self.spec.name.clone().unwrap_or_else(|| "default".into()),
            self.backend.clone(),
        );
        engine.up(&[], true, false, false).await
    }

    pub async fn down(&self, handle: &ComposeHandle, volumes: bool) -> Result<(), ContainerError> {
        let mut engine = perry_container_compose::ComposeEngine::new(
            self.spec.clone(),
            handle.project_name.clone(),
            self.backend.clone(),
        );
        // Note: In a real impl, we'd restore the engine state (containers map) from the handle
        engine.down(volumes, false).await
    }

    pub async fn ps(&self, handle: &ComposeHandle) -> Result<Vec<ContainerInfo>, ContainerError> {
        let mut engine = perry_container_compose::ComposeEngine::new(
            self.spec.clone(),
            handle.project_name.clone(),
            self.backend.clone(),
        );
        engine.ps().await
    }

    pub async fn logs(&self, handle: &ComposeHandle, service: Option<&str>, tail: Option<u32>) -> Result<ContainerLogs, ContainerError> {
        let mut engine = perry_container_compose::ComposeEngine::new(
            self.spec.clone(),
            handle.project_name.clone(),
            self.backend.clone(),
        );
        engine.logs(service, tail).await
    }

    pub async fn exec(&self, handle: &ComposeHandle, service: &str, cmd: &[String]) -> Result<ContainerLogs, ContainerError> {
        let mut engine = perry_container_compose::ComposeEngine::new(
            self.spec.clone(),
            handle.project_name.clone(),
            self.backend.clone(),
        );
        engine.exec(service, cmd).await
    }
}
