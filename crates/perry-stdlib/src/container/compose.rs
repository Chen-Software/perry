//! ComposeWrapper — thin orchestration adapter over `perry_container_compose::ComposeEngine`.

use perry_container_compose::backend::ContainerBackend;
use super::types::{
    ComposeHandle, ComposeSpec, ContainerError,
};
use std::sync::Arc;
pub use perry_container_compose::ComposeEngine;

pub struct ComposeWrapper {
    pub engine: Arc<ComposeEngine>,
}

impl ComposeWrapper {
    pub fn new(spec: ComposeSpec, backend: Arc<dyn ContainerBackend>) -> Self {
        let project_name = spec.name.clone().unwrap_or_else(|| "perry-stack".to_string());

        Self {
            engine: Arc::new(ComposeEngine::new(spec, project_name, backend)),
        }
    }

    pub async fn up(&self) -> Result<ComposeHandle, ContainerError> {
        self.engine.up(&[], true, false, false).await.map_err(Into::into)
    }
}

/// Convenience function: create a `ComposeEngine` from `spec` and call `up()`.
pub async fn compose_up(
    spec: ComposeSpec,
    backend: Arc<dyn ContainerBackend>,
) -> Result<Arc<ComposeEngine>, ContainerError> {
    let wrapper = ComposeWrapper::new(spec, backend);
    wrapper.up().await.map_err(ContainerError::from)?;
    Ok(wrapper.engine)
}
