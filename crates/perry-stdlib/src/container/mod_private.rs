use std::sync::Arc;
use tokio::sync::OnceCell;
use perry_container_compose::backend::{detect_backend, ContainerBackend};
use crate::container::types::ContainerError;

static GLOBAL_BACKEND: OnceCell<Arc<dyn ContainerBackend>> = OnceCell::const_new();

pub async fn get_global_backend_instance() -> Result<Arc<dyn ContainerBackend>, ContainerError> {
    GLOBAL_BACKEND.get_or_try_init(|| async {
        let res = detect_backend().await;

        let driver = match res {
            Ok(d) => d,
            Err(perry_container_compose::error::ComposeError::NoBackendFound { probed }) => {
                // Try interactive installer
                match perry_container_compose::installer::BackendInstaller::run().await {
                    Ok(d) => d,
                    Err(_) => return Err(ContainerError::NoBackendFound { probed }),
                }
            }
            Err(e) => return Err(ContainerError::from(e)),
        };

        let b = Arc::from(driver.instantiate()) as Arc<dyn ContainerBackend>;
        Ok(b)
    }).await.map(Arc::clone)
}

pub fn get_cached_backend_name() -> &'static str {
    GLOBAL_BACKEND.get()
        .map(|b| b.backend_name())
        .unwrap_or("unknown")
}
