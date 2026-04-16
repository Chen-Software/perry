//! ComposeWrapper — thin orchestration adapter over `perry_container_compose::ComposeEngine`.

use perry_container_compose::backend::ContainerBackend;
use super::types::{
    ComposeSpec, ContainerError, ContainerInfo, ContainerLogs,
};
use std::sync::Arc;
use perry_container_compose::ComposeEngine;

pub async fn compose_up(spec: ComposeSpec, backend: Arc<dyn ContainerBackend>) -> Result<Arc<ComposeEngine>, ContainerError> {
    let project_name = spec.name.clone().unwrap_or_else(|| "perry-stack".to_string());
    let engine = Arc::new(ComposeEngine::new(spec, project_name, backend));
    engine.up(&[], true, false, false).await.map_err(ContainerError::from)?;
    Ok(engine)
}
