//! Container backend abstraction — re-exports from `perry_container_compose::backend`.

pub use perry_container_compose::backend::{
    ContainerBackend, OciBackend, BackendDriver, OciCommandBuilder,
    detect_backend, get_backend,
};
