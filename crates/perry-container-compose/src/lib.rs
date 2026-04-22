pub mod types;
pub mod error;
pub mod yaml;
pub mod project;
pub mod service;
pub mod compose;
pub mod backend;
pub mod cli;
pub mod config;
pub mod installer;

#[cfg(feature = "ffi")]
pub mod ffi;

pub use error::{ComposeError, Result, BackendProbeResult};
pub use types::{ComposeSpec, ComposeService, ComposeHandle};
pub use compose::{ComposeEngine, WorkloadGraphEngine};
pub use project::ComposeProject;
pub use backend::{ContainerBackend, CliBackend, DockerProtocol, AppleContainerProtocol, LimaProtocol, MockBackend, detect_backend};
