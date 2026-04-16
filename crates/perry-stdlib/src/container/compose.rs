use super::types::{ComposeHandle, ComposeSpec};
use super::backend::ContainerBackend;
use super::types::{ContainerInfo, ContainerLogs};
use std::sync::Arc;
use perry_container_compose::compose::{get_engine, take_engine};

pub struct ComposeWrapper {
    spec: ComposeSpec,
    backend: Arc<dyn ContainerBackend>,
}

impl ComposeWrapper {
    pub fn new(spec: ComposeSpec, backend: Arc<dyn ContainerBackend>) -> Self {
        Self { spec, backend }
    }

    pub async fn up(&self) -> Result<ComposeHandle, String> {
        let engine = Arc::new(perry_container_compose::ComposeEngine::new(
            self.spec.clone(),
            self.spec.name.clone().unwrap_or_else(|| "perry".into()),
            Arc::clone(&self.backend),
        ));
        engine.up(&[], true, false, false).await.map_err(|e| e.to_string())
    }

    pub async fn down(&self, handle: &ComposeHandle, volumes: bool) -> Result<(), String> {
        let engine = take_engine(handle.stack_id)
            .ok_or_else(|| "Compose engine not found".to_string())?;
        engine.down(volumes, false).await.map_err(|e| e.to_string())
    }

    pub async fn ps(&self, handle: &ComposeHandle) -> Result<Vec<ContainerInfo>, String> {
        let engine = get_engine(handle.stack_id)
            .ok_or_else(|| "Compose engine not found".to_string())?;
        engine.ps().await.map_err(|e| e.to_string())
    }

    pub async fn logs(
        &self,
        handle: &ComposeHandle,
        service: Option<&str>,
        tail: Option<u32>,
    ) -> Result<ContainerLogs, String> {
        let engine = get_engine(handle.stack_id)
            .ok_or_else(|| "Compose engine not found".to_string())?;
        engine.logs(service, tail).await.map_err(|e| e.to_string())
    }

    pub async fn exec(
        &self,
        handle: &ComposeHandle,
        service: &str,
        cmd: &[String],
    ) -> Result<ContainerLogs, String> {
        let engine = get_engine(handle.stack_id)
            .ok_or_else(|| "Compose engine not found".to_string())?;
        engine.exec(service, cmd).await.map_err(|e| e.to_string())
    }

    pub async fn start(&self, handle: &ComposeHandle, services: &[String]) -> Result<(), String> {
        let engine = get_engine(handle.stack_id)
            .ok_or_else(|| "Compose engine not found".to_string())?;
        engine.start(services).await.map_err(|e| e.to_string())
    }

    pub async fn stop(&self, handle: &ComposeHandle, services: &[String]) -> Result<(), String> {
        let engine = get_engine(handle.stack_id)
            .ok_or_else(|| "Compose engine not found".to_string())?;
        engine.stop(services).await.map_err(|e| e.to_string())
    }

    pub async fn restart(&self, handle: &ComposeHandle, services: &[String]) -> Result<(), String> {
        let engine = get_engine(handle.stack_id)
            .ok_or_else(|| "Compose engine not found".to_string())?;
        engine.restart(services).await.map_err(|e| e.to_string())
    }
}

pub async fn compose_up(
    spec: ComposeSpec,
    backend: Arc<dyn ContainerBackend>,
) -> Result<ComposeHandle, String> {
    let wrapper = ComposeWrapper::new(spec, backend);
    wrapper.up().await
}
