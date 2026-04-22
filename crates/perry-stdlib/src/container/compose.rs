//! Thin stdlib wrapper around the `perry-container-compose` engine.

use std::sync::Arc;
use perry_container_compose::compose::ComposeEngine;
use perry_container_compose::types::{ComposeSpec, ComposeHandle};
use crate::container::backend::ContainerBackend;
use crate::container::types::ContainerError;

pub struct ComposeWrapper {
    pub engine: Arc<ComposeEngine>,
}

impl ComposeWrapper {
    pub fn new(spec: ComposeSpec, backend: Arc<dyn ContainerBackend>) -> Self {
        ComposeWrapper {
            engine: Arc::new(ComposeEngine::new(spec, "perry-stack".to_string(), backend)),
        }
    }

    pub async fn up(&self) -> Result<ComposeHandle, ContainerError> {
        self.engine.up(&[], true, false, false).await.map_err(ContainerError::from)
    }

    pub async fn down(&self, handle: &ComposeHandle, volumes: bool) -> Result<(), ContainerError> {
        let engine = ComposeEngine::get_engine(handle.stack_id).ok_or(ContainerError::NotFound("stack_id".into()))?;
        engine.down(&[], false, volumes).await.map_err(ContainerError::from)
    }

    pub async fn ps(&self, handle: &ComposeHandle) -> Result<Vec<perry_container_compose::types::ContainerInfo>, ContainerError> {
        let engine = ComposeEngine::get_engine(handle.stack_id).ok_or(ContainerError::NotFound("stack_id".into()))?;
        engine.ps().await.map_err(ContainerError::from)
    }

    pub async fn logs(&self, handle: &ComposeHandle, service: Option<&str>, tail: Option<u32>) -> Result<perry_container_compose::types::ContainerLogs, ContainerError> {
        let engine = ComposeEngine::get_engine(handle.stack_id).ok_or(ContainerError::NotFound("stack_id".into()))?;
        let svcs = service.map(|s| vec![s.to_string()]).unwrap_or_default();
        let logs_map = engine.logs(&svcs, tail).await.map_err(ContainerError::from)?;
        let mut stdout = String::new();
        let mut stderr = String::new();
        for (name, logs) in logs_map {
            stdout.push_str(&format!("--- {} ---\n{}", name, logs.stdout));
            stderr.push_str(&format!("--- {} ---\n{}", name, logs.stderr));
        }
        Ok(perry_container_compose::types::ContainerLogs { stdout, stderr })
    }

    pub async fn exec(&self, handle: &ComposeHandle, service: &str, cmd: &[String]) -> Result<perry_container_compose::types::ContainerLogs, ContainerError> {
        let engine = ComposeEngine::get_engine(handle.stack_id).ok_or(ContainerError::NotFound("stack_id".into()))?;
        engine.exec(service, cmd).await.map_err(ContainerError::from)
    }

    pub async fn start(&self, handle: &ComposeHandle, services: &[String]) -> Result<(), ContainerError> {
        let engine = ComposeEngine::get_engine(handle.stack_id).ok_or(ContainerError::NotFound("stack_id".into()))?;
        engine.start(services).await.map_err(ContainerError::from)
    }

    pub async fn stop(&self, handle: &ComposeHandle, services: &[String]) -> Result<(), ContainerError> {
        let engine = ComposeEngine::get_engine(handle.stack_id).ok_or(ContainerError::NotFound("stack_id".into()))?;
        engine.stop(services).await.map_err(ContainerError::from)
    }

    pub async fn restart(&self, handle: &ComposeHandle, services: &[String]) -> Result<(), ContainerError> {
        let engine = ComposeEngine::get_engine(handle.stack_id).ok_or(ContainerError::NotFound("stack_id".into()))?;
        engine.restart(services).await.map_err(ContainerError::from)
    }

    pub async fn config(&self) -> Result<ComposeSpec, ContainerError> {
        Ok(self.engine.spec.clone())
    }
}
