//! Re-exports from perry-container-compose for the container standard library.

pub use perry_container_compose::backend::{
    ContainerBackend, CliBackend, CliProtocol,
    DockerProtocol, AppleContainerProtocol, LimaProtocol,
    BackendProbeResult, detect_backend, probe_all_backends
};

use crate::container::types::ContainerError;
use std::sync::Arc;

/// stdlib-specific backend detection bridge.
pub async fn get_backend() -> Result<Arc<dyn ContainerBackend>, ContainerError> {
    detect_backend().await
        .map(Arc::from)
        .map_err(|probed| ContainerError::NoBackendFound { probed })
}
