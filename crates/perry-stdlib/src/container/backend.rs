//! Backend abstraction for container runtimes.

pub use perry_container_compose::backend::{
    BackendProbeResult, CliBackend, CliProtocol, ContainerBackend, detect_backend, DockerProtocol,
};
pub use perry_container_compose::types::{ComposeNetwork, ComposeVolume};

use super::types::ContainerError;
use std::sync::Arc;
use std::collections::HashMap;

pub async fn get_backend_async() -> Result<Arc<dyn ContainerBackend>, ContainerError> {
    detect_backend().await
        .map(|b| Arc::from(b) as Arc<dyn ContainerBackend>)
        .map_err(|probed| {
            let msg = probed.iter().map(|r| format!("{}: {}", r.name, r.reason)).collect::<Vec<_>>().join(", ");
            ContainerError::BackendError { code: 503, message: format!("No backend found: {}", msg) }
        })
}

// Deprecated: use get_backend_async instead.
// This still exists for synchronous initialization if needed, but should be avoided.
pub fn get_backend() -> Result<Arc<dyn ContainerBackend>, ContainerError> {
    let rt = tokio::runtime::Handle::try_current()
        .map(|h| h.clone())
        .unwrap_or_else(|_| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime")
                .handle()
                .clone()
        });
    rt.block_on(get_backend_async())
}
