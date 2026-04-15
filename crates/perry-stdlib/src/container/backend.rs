//! Container backend re-exports and detection.

pub use perry_container_compose::backend::{
    AppleContainerProtocol, CliBackend, CliProtocol, ContainerBackend,
    DockerProtocol, LimaProtocol, detect_backend,
};
pub use perry_container_compose::error::BackendProbeResult;
