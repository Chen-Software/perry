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
pub mod installer;
pub mod workload;
pub mod testing;

#[cfg(feature = "ffi")]
pub mod ffi;

pub use error::{ComposeError, Result, BackendProbeResult};
pub use types::{ComposeSpec, ComposeService, ComposeHandle};
pub use compose::{ComposeEngine, WorkloadGraphEngine};
pub use project::ComposeProject;
pub use backend::{ContainerBackend, CliBackend, CliProtocol, DockerProtocol, AppleContainerProtocol, LimaProtocol, detect_backend, OciBackend, BackendDriver, OciCommandBuilder, get_global_backend_instance};
pub use backend::{DockerBackend, AppleBackend, LimaBackend};
pub use yaml::{interpolate_yaml as interpolate, parse_dotenv, parse_compose_yaml};
