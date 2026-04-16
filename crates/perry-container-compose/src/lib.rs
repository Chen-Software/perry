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

pub use indexmap;

#[cfg(feature = "ffi")]
pub mod ffi;

pub use error::{ComposeError, Result};
pub use types::{ComposeHandle, ComposeService, ComposeSpec};
pub use compose::{ComposeEngine, resolve_startup_order};
pub use project::ComposeProject;
pub use backend::{ContainerBackend, OciBackend, BackendDriver, detect_backend};
