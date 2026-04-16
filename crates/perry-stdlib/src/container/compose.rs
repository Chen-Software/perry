use crate::container::get_global_backend;
use crate::container::types::ContainerError;
use perry_container_compose::compose::ComposeEngine;
use perry_container_compose::types::{ComposeHandle, ComposeSpec};
use std::sync::Arc;

pub async fn compose_up(
    spec: ComposeSpec,
) -> Result<(Arc<ComposeEngine>, ComposeHandle), ContainerError> {
    let backend = get_global_backend().await;
    // TODO: support project name resolution in stdlib
    let engine = Arc::new(ComposeEngine::new(spec, "perry".into(), backend));
    let handle = engine
        .up(&[], true, false, false)
        .await
        .map_err(|e| ContainerError::ServiceStartupFailed {
            service: "compose".into(),
            error: e.to_string(),
        })?;
    Ok((engine, handle))
}
