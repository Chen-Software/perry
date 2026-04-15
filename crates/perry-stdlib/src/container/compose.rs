//! Compose orchestration wrapper.

use super::types::ArcComposeEngine;
use perry_container_compose::types::{ComposeHandle, ComposeSpec};
use perry_container_compose::ComposeEngine;
use std::sync::Arc;
use crate::container::mod_private::get_global_backend_instance;

pub async fn compose_up(spec: ComposeSpec) -> Result<ComposeHandle, String> {
    let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
    let project_name = spec.name.clone().unwrap_or_else(|| "default".to_string());
    let engine = ComposeEngine::new(spec, project_name, Arc::clone(&backend) as Arc<dyn perry_container_compose::ContainerBackend>);

    let handle = engine.up(&[], true, false, false).await.map_err(|e| e.to_string())?;

    // We need to store the engine to perform operations on the handle later
    register_compose_handle_with_id(handle.stack_id, engine);

    Ok(handle)
}

fn register_compose_handle_with_id(id: u64, engine: ComposeEngine) {

    use dashmap::DashMap;
    use crate::container::types::COMPOSE_HANDLES;

    COMPOSE_HANDLES.get_or_init(DashMap::new).insert(id, ArcComposeEngine(Arc::new(engine)));
}
