use std::sync::Arc;
use perry_container_compose::ComposeEngine;
use perry_container_compose::error::Result;
use perry_container_compose::types::{ComposeSpec, ComposeHandle};
use crate::container::backend::detect_backend;

pub struct ComposeWrapper {
    pub engine: Arc<ComposeEngine>,
}

impl ComposeWrapper {
    pub async fn up(spec: ComposeSpec) -> Result<ComposeHandle> {
        let backend = detect_backend().await?;
        let project_name = spec.name.clone().unwrap_or_else(|| "perry-stack".into());
        let engine = Arc::new(ComposeEngine::new(spec, project_name, Arc::from(backend)));
        engine.up(false).await
    }
}
