use crate::container::types::*;
use crate::container::backend::ContainerBackend;
use std::sync::Arc;
use perry_container_compose::ComposeEngine;
use perry_container_compose::error::Result;

#[derive(Clone)]
pub struct ComposeWrapper {
    pub engine: Arc<ComposeEngine>,
}

impl ComposeWrapper {
    pub fn new(spec: ComposeSpec, backend: Arc<dyn ContainerBackend + Send + Sync>) -> Self {
        let project_name = spec.name.clone().unwrap_or_else(|| "default".into());

        Self {
            engine: Arc::new(ComposeEngine::new(spec, project_name, backend)),
        }
    }

    pub async fn up(&self) -> Result<ComposeHandle> {
        self.engine.up(&[], false, false, false).await
    }

    pub async fn down(&self, volumes: bool) -> Result<()> {
        self.engine.down(&[], false, volumes).await
    }

    pub async fn ps(&self) -> Result<Vec<ContainerInfo>> {
        self.engine.ps().await
    }

    pub async fn logs(&self, service: Option<&str>, tail: Option<u32>) -> Result<ContainerLogs> {
        let services = service.map(|s| vec![s.to_string()]).unwrap_or_default();
        let logs_map = self.engine.logs(&services, tail).await?;
        let mut stdout = String::new();
        for (name, log) in logs_map {
            stdout.push_str(&format!("{}: {}\n", name, log));
        }
        Ok(ContainerLogs { stdout, stderr: String::new() })
    }

    pub async fn exec(&self, service: &str, cmd: &[String]) -> Result<ContainerLogs> {
        self.engine.exec(service, cmd, None, None).await
    }

    pub async fn start(&self, services: &[String]) -> Result<()> {
        self.engine.start(services).await
    }

    pub async fn stop(&self, services: &[String]) -> Result<()> {
        self.engine.stop(services).await
    }

    pub async fn restart(&self, services: &[String]) -> Result<()> {
        self.engine.restart(services).await
    }

    pub async fn config(&self) -> Result<String> {
        self.engine.config()
    }

    pub fn get_handle(&self) -> ComposeHandle {
        ComposeHandle {
            stack_id: rand::random(),
            project_name: self.engine.project_name.clone(),
            services: self.engine.spec.services.keys().cloned().collect(),
        }
    }
}

pub async fn compose_up(spec: ComposeSpec, backend: Arc<dyn ContainerBackend + Send + Sync>) -> Result<ComposeWrapper> {
    let wrapper = ComposeWrapper::new(spec, backend);
    wrapper.up().await?;
    Ok(wrapper)
}
