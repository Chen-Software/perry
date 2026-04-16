pub use perry_container_compose::backend::{ContainerBackend, OciBackend, BackendDriver, detect_backend};
use crate::container::types::ContainerError;
use std::sync::{Arc, OnceLock};

static BACKEND: OnceLock<Arc<dyn ContainerBackend>> = OnceLock::new();

pub async fn get_global_backend() -> Result<Arc<dyn ContainerBackend>, ContainerError> {
    if let Some(b) = BACKEND.get() {
        return Ok(Arc::clone(b));
    }
    let b = detect_backend().await
        .map(Arc::from)
        .map_err(|probed| ContainerError::NoBackendFound { probed })?;
    let _ = BACKEND.set(Arc::clone(&b));
    Ok(b)
}

pub fn get_backend_name() -> &'static str {
    BACKEND.get().map(|b| b.backend_name()).unwrap_or("unknown")
}
