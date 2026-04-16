//! Backend abstraction for container runtimes.
//!
//! Re-exports the backend system from perry-container-compose.

pub use perry_container_compose::backend::{
    detect_backend, probe_all_backends, AppleBackend, AppleContainerProtocol, BackendProbeResult, CliBackend,
    CliProtocol, ContainerBackend, DockerBackend, DockerProtocol, LimaBackend, LimaProtocol,
    NetworkConfig, VolumeConfig,
};
