//! Container backend abstraction — re-exports from `perry_container_compose::backend`.
//!
//! This module re-exports the core backend types so that the rest of `perry-stdlib`
//! and downstream crates can use them without depending on `perry-container-compose`
//! directly.

use std::sync::Arc;
use super::types::ContainerError;

pub use perry_container_compose::backend::{
    AppleContainerProtocol, CliBackend, CliProtocol, ContainerBackend, DockerProtocol,
    LimaProtocol,
};

/// Synchronous best-effort backend selector.
///
/// Returns the first available container backend wrapped in an `Arc`.
/// Prefer `detect_backend().await` in async contexts.
pub fn get_backend() -> Result<Arc<dyn ContainerBackend>, ContainerError> {
    perry_container_compose::backend::get_container_backend()
        .map(|b| Arc::from(b) as Arc<dyn ContainerBackend>)
        .map_err(|e| ContainerError::BackendError {
            code: 1,
            message: e.to_string(),
        })
}
