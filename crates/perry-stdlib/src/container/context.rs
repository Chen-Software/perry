use perry_container_compose::backend::ContainerBackend;
use dashmap::DashMap;
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;

pub struct ContainerContext {
    pub backend: Arc<Mutex<Option<Arc<dyn ContainerBackend>>>>,
    pub handles: DashMap<u64, HandleEntry>,
}

pub enum HandleEntry {
    Container(perry_container_compose::types::ContainerHandle),
    Compose(Arc<perry_container_compose::ComposeEngine>),
    Graph(Arc<crate::container::workload::WorkloadGraphEngine>),
}

static GLOBAL_CONTEXT: OnceLock<ContainerContext> = OnceLock::new();

impl ContainerContext {
    pub fn global() -> &'static ContainerContext {
        GLOBAL_CONTEXT.get_or_init(|| Self::new())
    }

    pub fn new() -> Self {
        Self {
            backend: Arc::new(Mutex::new(None)),
            handles: DashMap::new(),
        }
    }
}
