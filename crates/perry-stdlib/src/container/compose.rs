//! Compose orchestration wrapper.

use super::types::{ArcComposeEngine, ContainerInfo, ContainerLogs};
use perry_container_compose::types::{ComposeHandle, ComposeSpec};
use perry_container_compose::ComposeEngine;
use std::sync::Arc;
use crate::container::get_global_backend;
use crate::container::types::COMPOSE_HANDLES;
use dashmap::DashMap;

pub struct ComposeWrapper {
    spec: ComposeSpec,
    backend: Arc<dyn perry_container_compose::ContainerBackend>,
}

impl ComposeWrapper {
    pub fn new(spec: ComposeSpec, backend: Arc<dyn perry_container_compose::ContainerBackend>) -> Self {
        Self { spec, backend }
    }

    pub async fn up(&self) -> Result<ComposeHandle, perry_container_compose::error::ComposeError> {
        let project_name = self.spec.name.clone().unwrap_or_else(|| "default".to_string());
        let engine = ComposeEngine::new(self.spec.clone(), project_name, Arc::clone(&self.backend));
        engine.up(&[], true, false, false).await
    }

    pub async fn down(&self, handle: &ComposeHandle, volumes: bool) -> Result<(), perry_container_compose::error::ComposeError> {
        let project_name = handle.project_name.clone();
        let engine = ComposeEngine::new(self.spec.clone(), project_name, Arc::clone(&self.backend));
        engine.down(volumes, false).await
    }

    pub async fn ps(&self, handle: &ComposeHandle) -> Result<Vec<ContainerInfo>, perry_container_compose::error::ComposeError> {
        let project_name = handle.project_name.clone();
        let engine = ComposeEngine::new(self.spec.clone(), project_name, Arc::clone(&self.backend));
        let infos = engine.ps().await?;
        Ok(infos.into_iter().map(|i| ContainerInfo {
            id: i.id,
            name: i.name,
            image: i.image,
            status: i.status,
            ports: i.ports,
            labels: i.labels,
            created: i.created,
        }).collect())
    }

    pub async fn logs(&self, handle: &ComposeHandle, service: Option<&str>, tail: Option<u32>) -> Result<ContainerLogs, perry_container_compose::error::ComposeError> {
        let project_name = handle.project_name.clone();
        let engine = ComposeEngine::new(self.spec.clone(), project_name, Arc::clone(&self.backend));
        let logs = engine.logs(service, tail).await?;
        Ok(ContainerLogs {
            stdout: logs.stdout,
            stderr: logs.stderr,
        })
    }

    pub async fn exec(&self, handle: &ComposeHandle, service: &str, cmd: &[String]) -> Result<ContainerLogs, perry_container_compose::error::ComposeError> {
        let project_name = handle.project_name.clone();
        let engine = ComposeEngine::new(self.spec.clone(), project_name, Arc::clone(&self.backend));
        let logs = engine.exec(service, cmd).await?;
        Ok(ContainerLogs {
            stdout: logs.stdout,
            stderr: logs.stderr,
        })
    }

    pub async fn config(&self) -> Result<ComposeSpec, perry_container_compose::error::ComposeError> {
        Ok(self.spec.clone())
    }

    pub async fn start(&self, handle: &ComposeHandle, services: &[String]) -> Result<(), perry_container_compose::error::ComposeError> {
        let project_name = handle.project_name.clone();
        let engine = ComposeEngine::new(self.spec.clone(), project_name, Arc::clone(&self.backend));
        engine.start(services).await
    }

    pub async fn stop(&self, handle: &ComposeHandle, services: &[String]) -> Result<(), perry_container_compose::error::ComposeError> {
        let project_name = handle.project_name.clone();
        let engine = ComposeEngine::new(self.spec.clone(), project_name, Arc::clone(&self.backend));
        engine.stop(services).await
    }

    pub async fn restart(&self, handle: &ComposeHandle, services: &[String]) -> Result<(), perry_container_compose::error::ComposeError> {
        let project_name = handle.project_name.clone();
        let engine = ComposeEngine::new(self.spec.clone(), project_name, Arc::clone(&self.backend));
        engine.restart(services).await
    }
}
