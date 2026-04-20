use crate::container::types::{ComposeSpec, ComposeHandle, register_compose_handle};
use perry_container_compose::ComposeEngine;
use perry_container_compose::backend::ContainerBackend;
use std::sync::Arc;

pub async fn compose_up(spec: ComposeSpec, backend: Arc<dyn ContainerBackend + Send + Sync>) -> Result<ComposeHandle, String> {
    let engine = ComposeEngine::new(spec, backend);
    let handle = engine.up(false).await.map_err(|e| e.to_string())?;
    register_compose_handle(engine);
    Ok(handle)
}
