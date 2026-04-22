use crate::container::types::{ComposeHandle, ComposeSpec};
use perry_container_compose::backend::ContainerBackend;
use perry_container_compose::compose::ComposeEngine;
use std::sync::Arc;

pub async fn compose_up(
    spec: ComposeSpec,
    project_name: String,
    backend: Arc<dyn ContainerBackend>,
) -> Result<u64, String> {
    let engine = ComposeEngine::new(spec, project_name, backend);
    match engine.up(&[], true, false, false).await {
        Ok(handle) => {
            // Requirement 6.1: Registers engine in COMPOSE_HANDLES
            let id = crate::container::types::register_compose_handle(engine);
            Ok(id)
        }
        Err(e) => Err(e.to_string()),
    }
}
