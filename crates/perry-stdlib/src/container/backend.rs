//! Container backend re-exports and selection.

pub use perry_container_compose::backend::{
    detect_backend, ContainerBackend, NetworkConfig, VolumeConfig,
};
pub use perry_container_compose::error::BackendProbeResult;
use std::sync::Arc;

pub fn get_backend() -> Result<Arc<dyn ContainerBackend + Send + Sync>, super::types::ContainerError> {
    tokio::runtime::Handle::current().block_on(async {
        let b = perry_container_compose::backend::detect_backend().await
            .map_err(|e| super::types::ContainerError::BackendError { code: 1, message: e.to_string() })?;
        let arc: Arc<dyn ContainerBackend + Send + Sync> = Arc::from(b as Box<dyn ContainerBackend + Send + Sync>);
        Ok(arc)
    })
}
