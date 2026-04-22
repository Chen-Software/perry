use perry_container_compose::backend::ContainerBackend;
use std::sync::{Arc, OnceLock};
use dashmap::DashMap;
use tokio::sync::Mutex;

pub enum HandleEntry {
    Container(perry_container_compose::types::ContainerHandle),
    Compose(perry_container_compose::types::ComposeHandle),
    Graph(perry_container_compose::types::ComposeHandle), // Using ComposeHandle for now
}

pub struct ContainerContext {
    pub backend: OnceLock<Arc<dyn ContainerBackend>>,
    pub backend_init_mutex: Mutex<()>,
    pub handles: DashMap<u64, HandleEntry>,
}

impl ContainerContext {
    pub fn new() -> Self {
        Self {
            backend: OnceLock::new(),
            backend_init_mutex: Mutex::const_new(()),
            handles: DashMap::new(),
        }
    }

    pub fn global() -> &'static Self {
        static GLOBAL: OnceLock<ContainerContext> = OnceLock::new();
        GLOBAL.get_or_init(Self::new)
    }
}
