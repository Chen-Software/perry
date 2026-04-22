use std::collections::HashMap;
use crate::container::types::*;
use crate::container::backend::ContainerBackend;
use std::sync::Arc;
use perry_container_compose::error::{ComposeError, Result};
use perry_container_compose::compose::ComposeEngine as Engine;

#[derive(Clone)]
pub struct ComposeEngine {
    inner: Arc<Engine>,
}

impl ComposeEngine {
    pub fn new(spec: ComposeSpec, backend: Arc<dyn ContainerBackend>) -> Self {
        Self { inner: Arc::new(Engine::new(spec, "default".into(), backend)) }
    }

    pub async fn up(&self) -> Result<ComposeHandle> {
        self.inner.up(&[], true, false, false).await
    }

    pub async fn down(&self, volumes: bool) -> Result<()> {
        self.inner.down(&[], false, volumes).await
    }

    pub async fn ps(&self) -> Result<Vec<ContainerInfo>> {
        self.inner.ps().await
    }

    pub async fn logs(&self, service: Option<&str>, tail: Option<u32>) -> Result<ContainerLogs> {
        let svcs = service.map(|s| vec![s.to_string()]).unwrap_or_default();
        let logs_map = self.inner.logs(&svcs, tail).await?;
        let mut stdout = String::new();
        for (_, l) in logs_map {
            stdout.push_str(&l);
        }
        Ok(ContainerLogs { stdout, stderr: "".into() })
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

pub async fn compose_up(spec: ComposeSpec, backend: Arc<dyn ContainerBackend>) -> Result<(ComposeHandle, ComposeEngine)> {
    let engine = ComposeEngine::new(spec, backend);
    let handle = engine.up().await?;
    Ok((handle, engine))
}
