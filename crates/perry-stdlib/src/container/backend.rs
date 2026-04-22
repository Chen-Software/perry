//! Backend abstraction for container runtimes

pub use perry_container_compose::backend::{
    detect_backend, BackendDriver, BackendProbeResult, ContainerBackend, OciBackend,
    OciCommandBuilder,
};
