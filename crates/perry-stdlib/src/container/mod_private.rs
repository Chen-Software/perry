use std::sync::Arc;
use tokio::sync::OnceCell;
use perry_container_compose::backend::{detect_backend, ContainerBackend};
use crate::container::types::ContainerError;

static GLOBAL_BACKEND: OnceCell<Arc<dyn ContainerBackend>> = OnceCell::const_new();

pub async fn get_global_backend_instance() -> Result<Arc<dyn ContainerBackend>, ContainerError> {
    GLOBAL_BACKEND.get_or_try_init(|| async {
        let b = detect_backend().await
            .map(|d| Arc::from(d.instantiate()) as Arc<dyn ContainerBackend>)
            .map_err(ContainerError::from)?;
        Ok(b)
    }).await.map(Arc::clone)
}

pub fn get_cached_backend_name() -> &'static str {
    GLOBAL_BACKEND.get()
        .map(|b| b.backend_name())
        .unwrap_or("unknown")
}
