//! Re-exports and refinements of the container backend from the compose crate.

pub use perry_container_compose::backend::{
    detect_backend, AppleContainerProtocol, CliBackend, CliProtocol, ContainerBackend,
    DockerProtocol, LimaProtocol, NetworkConfig, VolumeConfig,
};

pub use perry_container_compose::{AppleBackend, DockerBackend, LimaBackend};
