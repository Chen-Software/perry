//! Backend abstraction for container runtimes.
//!
//! Re-exports ContainerBackend, CliBackend, CliProtocol, DockerProtocol,
//! AppleContainerProtocol, LimaProtocol from perry-container-compose

pub use perry_container_compose::backend::{
    ContainerBackend, CliBackend, CliProtocol,
    DockerProtocol, AppleContainerProtocol, LimaProtocol,
    DockerBackend, AppleBackend, LimaBackend,
    NetworkConfig, VolumeConfig, BuildConfig,
    BackendProbeResult, detect_backend,
};
