//! thin wrapper calling perry_container_compose::ComposeEngine

use std::sync::Arc;
use perry_container_compose::ComposeEngine;
use super::types::{ComposeSpec, ComposeHandle, ContainerError};
use super::backend::ContainerBackend;

pub async fn compose_up(spec: ComposeSpec, backend: Arc<dyn ContainerBackend>) -> Result<ComposeHandle, ContainerError> {
    let project_name = spec.name.clone().unwrap_or_else(|| "perry-compose".to_string());
    let engine = ComposeEngine::new(spec, project_name, backend);
    engine.up(&[], true, false, false).await.map_err(ContainerError::from)
}
