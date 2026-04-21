use std::sync::Arc;
pub use perry_container_compose::compose::ComposeEngine;
use perry_container_compose::types::{ComposeSpec, ComposeHandle};
use crate::container::backend::ContainerBackend;
use crate::container::error::ContainerError;

pub struct ComposeWrapper {
    pub engine: Arc<ComposeEngine>,
}

pub async fn compose_up(spec: ComposeSpec, project_name: String, backend: Arc<dyn ContainerBackend>) -> Result<ComposeHandle, ContainerError> {
    let engine = ComposeEngine::new(spec, project_name, backend);
    engine.up(true, false, false).await.map_err(|e| ContainerError::BackendError {
        code: 1,
        message: e.to_string()
    })
}
