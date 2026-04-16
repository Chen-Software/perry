use crate::container::types::{ComposeSpec, ContainerInfo, ContainerLogs};
use perry_container_compose::compose::ComposeEngine;
use perry_container_compose::backend::ContainerBackend;
use perry_container_compose::types::ComposeHandle;
use std::sync::Arc;
use std::collections::HashMap;

pub struct ComposeWrapper {
    engine: ComposeEngine,
}

impl ComposeWrapper {
    pub fn new(spec: ComposeSpec, backend: Arc<dyn ContainerBackend>) -> Self {
        Self { engine: ComposeEngine::new(spec, "default".into(), backend) }
    }
    pub async fn up(&self) -> Result<ComposeHandle, crate::container::types::ContainerError> {
        self.engine.up(&[], true, false, false).await
    }
    pub async fn down(&self, handle: &ComposeHandle, volumes: bool) -> Result<(), crate::container::types::ContainerError> {
        self.engine.down(&handle.services, false, volumes).await
    }
    pub async fn ps(&self, _handle: &ComposeHandle) -> Result<Vec<ContainerInfo>, crate::container::types::ContainerError> {
        self.engine.ps().await
    }
    pub async fn logs(&self, _handle: &ComposeHandle, service: Option<&str>, tail: Option<u32>) -> Result<ContainerLogs, crate::container::types::ContainerError> {
        let svcs = service.map(|s| vec![s.to_string()]).unwrap_or_default();
        let logs = self.engine.logs(&svcs, tail).await?;
        let stdout = logs.values().cloned().collect::<Vec<_>>().join("\n");
        Ok(ContainerLogs { stdout, stderr: "".into() })
    }
    pub async fn exec(&self, _handle: &ComposeHandle, service: &str, cmd: &[String]) -> Result<ContainerLogs, crate::container::types::ContainerError> {
        self.engine.exec(service, cmd, None, None).await
    }
    pub fn config(&self) -> Result<String, crate::container::types::ContainerError> {
        self.engine.config()
    }
    pub async fn start(&self, _handle: &ComposeHandle, services: &[String]) -> Result<(), crate::container::types::ContainerError> {
        self.engine.start(services).await
    }
    pub async fn stop(&self, _handle: &ComposeHandle, services: &[String]) -> Result<(), crate::container::types::ContainerError> {
        self.engine.stop(services).await
    }
    pub async fn restart(&self, _handle: &ComposeHandle, services: &[String]) -> Result<(), crate::container::types::ContainerError> {
        self.engine.restart(services).await
    }
}
