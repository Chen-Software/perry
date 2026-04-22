use crate::container::types::*;
use crate::container::backend::ContainerBackend;
use std::sync::Arc;
use perry_container_compose::error::Result;
use perry_container_compose::ComposeEngine as InnerEngine;

#[derive(Clone)]
pub struct ComposeEngine {
    inner: Arc<InnerEngine>,
}

impl ComposeEngine {
    pub fn new(spec: ComposeSpec, project_name: String, backend: Arc<dyn ContainerBackend>) -> Self {
        Self {
            inner: Arc::new(InnerEngine::new(spec, project_name, backend))
        }
    }

    pub async fn up(&self) -> Result<ComposeHandle> {
        self.inner.up(&[], false, false, false).await
    }

    pub async fn down(&self, volumes: bool) -> Result<()> {
        self.inner.down(&[], false, volumes).await
    }

    pub async fn ps(&self) -> Result<Vec<ContainerInfo>> {
        self.inner.ps().await
    }

    pub async fn logs(&self, service: Option<&str>, tail: Option<u32>) -> Result<ContainerLogs> {
        let services = service.map(|s| vec![s.to_string()]).unwrap_or_default();
        self.inner.logs(&services, tail).await
    }

    pub async fn exec(&self, service: &str, cmd: &[String]) -> Result<ContainerLogs> {
        self.inner.exec(service, cmd, None, None).await
    }

    pub async fn start(&self, services: &[String]) -> Result<()> {
        self.inner.start(services).await
    }

    pub async fn stop(&self, services: &[String]) -> Result<()> {
        self.inner.stop(services).await
    }

    pub async fn restart(&self, services: &[String]) -> Result<()> {
        self.inner.restart(services).await
    }

    pub fn config(&self) -> Result<String> {
        self.inner.config()
    }
}
