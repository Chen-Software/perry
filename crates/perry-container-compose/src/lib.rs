pub mod types;
pub mod error;
pub mod yaml;
pub mod project;
pub mod service;
pub mod compose;
pub mod backend;
pub mod cli;
pub mod config;
pub mod orchestrate;
pub mod commands;
#[cfg(feature = "installer")]
pub mod installer;
pub mod testing;

#[cfg(feature = "ffi")]
pub mod ffi;

pub use error::{ComposeError, Result, BackendProbeResult};
pub use types::{ContainerCompose, ComposeService, ComposeHandle};
pub use indexmap;
pub use compose::ComposeEngine;
pub use project::ComposeProject;
pub use backend::{ContainerBackend, CliBackend, CliProtocol, DockerProtocol, AppleContainerProtocol, LimaProtocol, detect_backend};
