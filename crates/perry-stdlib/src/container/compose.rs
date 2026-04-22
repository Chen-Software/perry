//! Compose orchestration wrapper.

use perry_container_compose::backend::ContainerBackend;
use super::types::{
    ComposeHandle, ComposeSpec, ContainerError, ContainerInfo, ContainerLogs,
};
use std::sync::Arc;
use crate::container::get_global_backend_instance;
use crate::container::types::COMPOSE_HANDLES;
use dashmap::DashMap;

pub async fn compose_up(spec: ComposeSpec) -> Result<ComposeHandle, String> {
    let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
    let project_name = spec.name.clone().unwrap_or_else(|| "default".to_string());
    let engine = Arc::new(ComposeEngine::new(spec, project_name, Arc::clone(&backend)));

    let handle = Arc::clone(&engine).up(&[], true, false, false).await.map_err(|e| e.to_string())?;

    Ok(handle)
}

pub async fn compose_down(id: u64, volumes: bool) -> Result<(), String> {
    let engine = ComposeEngine::get_engine(id)
        .ok_or_else(|| format!("Compose stack {} not found", id))?;

    engine.down(&[], false, volumes).await.map_err(|e| e.to_string())?;
    ComposeEngine::unregister(id);
    Ok(())
}

pub async fn compose_ps(id: u64) -> Result<Vec<ContainerInfo>, String> {
    let engine = ComposeEngine::get_engine(id)
        .ok_or_else(|| format!("Compose stack {} not found", id))?;

    let infos = engine.ps().await.map_err(|e| e.to_string())?;
    Ok(infos.into_iter().map(|i| ContainerInfo {
        id: i.id,
        name: i.name,
        image: i.image,
        status: i.status,
        ports: i.ports,
        created: i.created,
        labels: i.labels,
    }).collect())
}

pub async fn compose_logs(id: u64, service: Option<String>, tail: Option<u32>) -> Result<ContainerLogs, String> {
    let engine = ComposeEngine::get_engine(id)
        .ok_or_else(|| format!("Compose stack {} not found", id))?;

    let services = service.map(|s| vec![s]).unwrap_or_default();
    let logs_map = engine.logs(&services, tail).await.map_err(|e| e.to_string())?;

    let mut stdout = String::new();
    let mut stderr = String::new();

    for (svc, logs) in logs_map {
        stdout.push_str(&format!("[{}] {}\n", svc, logs.stdout));
        stderr.push_str(&format!("[{}] {}\n", svc, logs.stderr));
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

    pub async fn logs(&self, _handle: &ComposeHandle, service: Option<&str>, tail: Option<u32>) -> Result<ContainerLogs, ContainerError> {
        let services = service.map(|s| vec![s.to_string()]).unwrap_or_default();
        let logs_map = self.engine.logs(&services, tail).await.map_err(ContainerError::from)?;

        let mut stdout = String::new();
        let mut stderr = String::new();

        for (svc, logs) in logs_map {
            stdout.push_str(&format!("[{}] {}\n", svc, logs));
        }

        Ok(ContainerLogs { stdout, stderr })
    }

    pub async fn exec(&self, _handle: &ComposeHandle, service: &str, cmd: &[String]) -> Result<ContainerLogs, ContainerError> {
        self.engine.exec(service, cmd, None, None).await.map_err(Into::into)
    }
}
