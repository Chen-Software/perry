pub use perry_container_compose::compose::ComposeEngine;
use perry_container_compose::types::{ComposeSpec, ComposeHandle};
use crate::container::backend::ContainerBackend;
use std::sync::Arc;
use perry_container_compose::error::Result;

pub async fn compose_up(spec: ComposeSpec, backend: Arc<dyn ContainerBackend + Send + Sync>) -> Result<ComposeHandle> {
    let project_name = spec.name.clone().unwrap_or_else(|| "default".into());
    let engine = ComposeEngine::new(spec, project_name, backend);
    engine.up(&[], false, false, false).await
}
