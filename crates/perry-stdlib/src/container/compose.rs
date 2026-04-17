use std::sync::Arc;
use perry_container_compose::ComposeEngine;
pub use perry_container_compose::types::{ComposeSpec, ComposeHandle};
pub use perry_container_compose::error::ComposeError;

pub async fn compose_up(spec: ComposeSpec, project_name: String, backend: Arc<dyn perry_container_compose::backend::ContainerBackend>) -> Result<Arc<ComposeEngine>, ComposeError> {
    let engine = Arc::new(ComposeEngine::new(spec, project_name, backend));
    // Up logic is already in ComposeEngine
    Ok(engine)
}
