//! Backend abstraction for container runtimes.

pub use perry_container_compose::backend::{
    ContainerBackend, NetworkConfig, VolumeConfig, detect_backend,
};
pub use perry_container_compose::error::BackendProbeResult;
use perry_container_compose::error::ComposeError;
use std::sync::Arc;

pub struct BackendAdapter {
    pub inner: Arc<dyn ContainerBackend>,
}

pub fn to_compose_err(e: ComposeError) -> ComposeError { e }
