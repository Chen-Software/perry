//! `perry-container-compose` — Docker Compose-like experience for Apple Container / Podman.

pub mod backend;
pub mod cli;
pub mod compose;
pub mod config;
pub mod error;
pub mod project;
pub mod service;
pub mod types;
pub mod yaml;

// FFI exports (Perry TypeScript integration)
#[cfg(feature = "ffi")]
pub mod ffi;

// Re-exports
pub use error::{BackendProbeResult, ComposeError, Result};
pub use types::{ComposeHandle, ComposeService, ComposeSpec};
pub use compose::{resolve_startup_order, ComposeEngine};
pub use indexmap::IndexMap;
pub use project::ComposeProject;
pub use backend::{
    detect_backend, AppleContainerProtocol, CliBackend, CliProtocol,
    ContainerBackend, DockerProtocol, LimaProtocol, NetworkConfig,
    VolumeConfig,
};

pub type DockerBackend = CliBackend<DockerProtocol>;
pub type AppleBackend = CliBackend<AppleContainerProtocol>;
pub type LimaBackend = CliBackend<LimaProtocol>;
