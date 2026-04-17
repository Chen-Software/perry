use crate::container::types::{ComposeSpec, ContainerInfo, ContainerLogs};
use perry_container_compose::compose::{ComposeEngine, get_compose_engine};
use perry_container_compose::backend::ContainerBackend;
use perry_container_compose::types::ComposeHandle;
use std::sync::Arc;

pub struct ComposeWrapper {
    spec: ComposeSpec,
    backend: Arc<dyn ContainerBackend>,
}

impl ComposeWrapper {
    pub fn new(spec: ComposeSpec, backend: Arc<dyn ContainerBackend>) -> Self {
        Self { spec, backend }
    }
    pub async fn up(&self) -> Result<ComposeHandle, crate::container::types::ContainerError> {
        let engine = ComposeEngine::new(self.spec.clone(), "default".into(), Arc::clone(&self.backend));
        engine.up(&[], true, false, false).await
    }
    pub async fn down(&self, handle: &ComposeHandle, volumes: bool) -> Result<(), crate::container::types::ContainerError> {
        if let Some(engine) = get_compose_engine(handle.stack_id) {
            engine.down(&handle.services, false, volumes).await
        } else {
            Err(crate::container::types::ContainerError::NotFound(format!("Stack ID {}", handle.stack_id)))
        }
    }
    pub async fn ps(&self, handle: &ComposeHandle) -> Result<Vec<ContainerInfo>, crate::container::types::ContainerError> {
        if let Some(engine) = get_compose_engine(handle.stack_id) {
            engine.ps().await
        } else {
            Err(crate::container::types::ContainerError::NotFound(format!("Stack ID {}", handle.stack_id)))
        }
    }
    pub async fn logs(&self, handle: &ComposeHandle, service: Option<&str>, tail: Option<u32>) -> Result<ContainerLogs, crate::container::types::ContainerError> {
        if let Some(engine) = get_compose_engine(handle.stack_id) {
            let svcs = service.map(|s| vec![s.to_string()]).unwrap_or_default();
            let logs = engine.logs(&svcs, tail).await?;
            let stdout = logs.values().cloned().collect::<Vec<_>>().join("\n");
            Ok(ContainerLogs { stdout, stderr: "".into() })
        } else {
            Err(crate::container::types::ContainerError::NotFound(format!("Stack ID {}", handle.stack_id)))
        }
    }
    pub async fn exec(&self, handle: &ComposeHandle, service: &str, cmd: &[String]) -> Result<ContainerLogs, crate::container::types::ContainerError> {
        if let Some(engine) = get_compose_engine(handle.stack_id) {
            engine.exec(service, cmd, None, None).await
        } else {
            Err(crate::container::types::ContainerError::NotFound(format!("Stack ID {}", handle.stack_id)))
        }
    }
    pub fn config(&self) -> Result<String, crate::container::types::ContainerError> {
        let engine = ComposeEngine::new(self.spec.clone(), "default".into(), Arc::clone(&self.backend));
        engine.config()
    }
    pub async fn start(&self, handle: &ComposeHandle, services: &[String]) -> Result<(), crate::container::types::ContainerError> {
        if let Some(engine) = get_compose_engine(handle.stack_id) {
            engine.start(services).await
        } else {
            Err(crate::container::types::ContainerError::NotFound(format!("Stack ID {}", handle.stack_id)))
        }
    }
    pub async fn stop(&self, handle: &ComposeHandle, services: &[String]) -> Result<(), crate::container::types::ContainerError> {
        if let Some(engine) = get_compose_engine(handle.stack_id) {
            engine.stop(services).await
        } else {
            Err(crate::container::types::ContainerError::NotFound(format!("Stack ID {}", handle.stack_id)))
        }
    }
    pub async fn restart(&self, handle: &ComposeHandle, services: &[String]) -> Result<(), crate::container::types::ContainerError> {
        if let Some(engine) = get_compose_engine(handle.stack_id) {
            engine.restart(services).await
        } else {
            Err(crate::container::types::ContainerError::NotFound(format!("Stack ID {}", handle.stack_id)))
        }
    }
}
