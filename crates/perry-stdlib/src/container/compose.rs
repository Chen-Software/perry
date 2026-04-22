use crate::types::{ComposeSpec, ComposeHandle, ContainerInfo, ContainerLogs};
use perry_container_compose::compose::ComposeEngine as Engine;
use std::sync::Arc;
use crate::backend::ContainerBackend;

pub type ComposeEngine = Engine;

pub async fn compose_up(spec: ComposeSpec, backend: Arc<dyn ContainerBackend + Send + Sync>) -> Result<ComposeHandle, perry_container_compose::error::ComposeError> {
    let engine = Engine::new(spec, backend);
    engine.up().await
}
