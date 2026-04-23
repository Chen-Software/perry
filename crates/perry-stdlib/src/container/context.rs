use perry_container_compose::backend::{detect_backend, ContainerBackend};
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;
use dashmap::DashMap;
use crate::container::types::{ContainerHandle, ComposeHandle};

pub enum HandleEntry {
    Container(ContainerHandle),
    Compose(Arc<perry_container_compose::ComposeEngine>),
    WorkloadGraph(Arc<crate::container::workload::WorkloadGraphState>),
}

pub struct ContainerContext {
    pub backend: OnceLock<Arc<dyn ContainerBackend>>,
    pub backend_mutex: Mutex<()>,
    pub handles: DashMap<u64, HandleEntry>,
}

static GLOBAL_CONTEXT: OnceLock<ContainerContext> = OnceLock::new();

impl ContainerContext {
    pub fn global() -> &'static ContainerContext {
        GLOBAL_CONTEXT.get_or_init(Self::new)
    }

    pub fn new() -> Self {
        Self {
            backend: OnceLock::new(),
            backend_mutex: Mutex::new(()),
            handles: DashMap::new(),
        }
    }

    pub async fn get_global_backend_instance(&self) -> Result<Arc<dyn ContainerBackend>, String> {
        if let Some(b) = self.backend.get() {
            return Ok(Arc::clone(b));
        }

        let _guard = self.backend_mutex.lock().await;
        if let Some(b) = self.backend.get() {
            return Ok(Arc::clone(b));
        }

        match detect_backend().await {
            Ok((_driver, b)) => {
                let arc_b: Arc<dyn ContainerBackend> = Arc::from(b);
                let _ = self.backend.set(Arc::clone(&arc_b));
                Ok(arc_b)
            }
            Err(probed) => Err(format!("No container backend found. Probed: {:?}", probed)),
        }
    }
}
