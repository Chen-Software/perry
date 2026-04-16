//! `perry-container-compose` — Docker Compose-like experience for Apple Container / Podman.
//!
//! Can be used:
//!
//! 1. As a standalone CLI binary (`perry-compose`)
//! 2. As a library imported from Perry TypeScript applications
//! 3. Via FFI from compiled Perry TypeScript code (requires `ffi` feature)

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
pub use backend::{
    detect_backend, AppleContainerProtocol, BackendProbeResult, CliBackend, CliProtocol,
    ContainerBackend, DockerProtocol, LimaProtocol,
};
pub use compose::ComposeEngine;
pub use error::{ComposeError, Result};
pub use project::ComposeProject;
pub use types::{ComposeHandle, ComposeService, ComposeSpec};
