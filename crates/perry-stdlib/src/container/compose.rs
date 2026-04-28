//! Compose orchestration wrapper.

use super::types::{ArcComposeEngine, ContainerInfo, ContainerLogs, COMPOSE_HANDLES};
use perry_container_compose::types::{ComposeHandle, ComposeSpec};
use perry_container_compose::ComposeEngine;
use std::sync::Arc;
use dashmap::DashMap;

pub async fn compose_up(spec: ComposeSpec, backend: Arc<dyn perry_container_compose::ContainerBackend>) -> Result<ComposeHandle, String> {
    let project_name = spec.name.clone().unwrap_or_else(|| "default".to_string());
    let engine = Arc::new(ComposeEngine::new(spec, project_name, backend));

    let handle = Arc::clone(&engine).up(&[], true, false, false).await.map_err(|e| e.to_string())?;
    COMPOSE_HANDLES.get_or_init(DashMap::new).insert(handle.stack_id, ArcComposeEngine(engine));
    Ok(handle)
}

pub async fn compose_down(id: u64, volumes: bool) -> Result<(), String> {
    let engine = {
        let handles = COMPOSE_HANDLES.get_or_init(DashMap::new);
        handles.get(&id).map(|e| Arc::clone(&e.0))
    }.ok_or_else(|| format!("Compose stack {} not found", id))?;

    engine.down(&[], false, volumes).await.map_err(|e| e.to_string())?;
    COMPOSE_HANDLES.get_or_init(DashMap::new).remove(&id);
    Ok(())
}

pub async fn compose_ps(id: u64) -> Result<Vec<ContainerInfo>, String> {
    let engine = {
        let handles = COMPOSE_HANDLES.get_or_init(DashMap::new);
        handles.get(&id).map(|e| Arc::clone(&e.0))
    }.ok_or_else(|| format!("Compose stack {} not found", id))?;

    let infos = engine.ps().await.map_err(|e| e.to_string())?;
    Ok(infos)
}

pub async fn compose_logs(id: u64, service: Option<String>, tail: Option<u32>) -> Result<ContainerLogs, String> {
    let engine = {
        let handles = COMPOSE_HANDLES.get_or_init(DashMap::new);
        handles.get(&id).map(|e| Arc::clone(&e.0))
    }.ok_or_else(|| format!("Compose stack {} not found", id))?;

    let logs = engine.logs(service.as_deref(), tail).await.map_err(|e| e.to_string())?;
    Ok(logs)
}

pub async fn compose_exec(id: u64, service: String, cmd: Vec<String>) -> Result<ContainerLogs, String> {
    let engine = {
        let handles = COMPOSE_HANDLES.get_or_init(DashMap::new);
        handles.get(&id).map(|e| Arc::clone(&e.0))
    }.ok_or_else(|| format!("Compose stack {} not found", id))?;

    let logs = engine.exec(&service, &cmd).await.map_err(|e| e.to_string())?;
    Ok(logs)
}

pub async fn compose_config(id: u64) -> Result<String, String> {
    let engine = {
        let handles = COMPOSE_HANDLES.get_or_init(DashMap::new);
        handles.get(&id).map(|e| Arc::clone(&e.0))
    }.ok_or_else(|| format!("Compose stack {} not found", id))?;

    engine.config().map_err(|e| e.to_string())
}

pub async fn compose_start(id: u64, services: Vec<String>) -> Result<(), String> {
    let engine = {
        let handles = COMPOSE_HANDLES.get_or_init(DashMap::new);
        handles.get(&id).map(|e| Arc::clone(&e.0))
    }.ok_or_else(|| format!("Compose stack {} not found", id))?;

    engine.start(&services).await.map_err(|e| e.to_string())
}

pub async fn compose_stop(id: u64, services: Vec<String>) -> Result<(), String> {
    let engine = {
        let handles = COMPOSE_HANDLES.get_or_init(DashMap::new);
        handles.get(&id).map(|e| Arc::clone(&e.0))
    }.ok_or_else(|| format!("Compose stack {} not found", id))?;

    engine.stop(&services).await.map_err(|e| e.to_string())
}

pub async fn compose_restart(id: u64, services: Vec<String>) -> Result<(), String> {
    let engine = {
        let handles = COMPOSE_HANDLES.get_or_init(DashMap::new);
        handles.get(&id).map(|e| Arc::clone(&e.0))
    }.ok_or_else(|| format!("Compose stack {} not found", id))?;

    engine.restart(&services).await.map_err(|e| e.to_string())
}
