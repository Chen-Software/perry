use std::sync::{Arc, OnceLock};
use dashmap::DashMap;
use perry_container_compose::ContainerBackend;
use tokio::sync::Mutex;
use crate::container::types::{
    ContainerHandle, ContainerInfo, ContainerLogs, ImageInfo, ArcComposeEngine
};

pub enum HandleEntry {
    Container(ContainerHandle),
    Compose(ArcComposeEngine),
    Workload(Arc<perry_container_compose::WorkloadGraphEngine>),
    ContainerInfoList(Vec<ContainerInfo>),
    ContainerInfo(ContainerInfo),
    ContainerLogs(ContainerLogs),
    ImageInfoList(Vec<ImageInfo>),
}

pub struct ContainerContext {
    pub backend: OnceLock<Arc<dyn ContainerBackend>>,
    pub backend_init_lock: Mutex<()>,
    pub handles: DashMap<u64, HandleEntry>,
}

impl ContainerContext {
    pub fn new() -> Self {
        Self {
            backend: OnceLock::new(),
            backend_init_lock: Mutex::new(()),
            handles: DashMap::new(),
        }
    }

    pub fn global() -> &'static Self {
        static GLOBAL: OnceLock<ContainerContext> = OnceLock::new();
        GLOBAL.get_or_init(Self::new)
    }

    pub async fn get_backend(&self) -> Result<Arc<dyn ContainerBackend>, perry_container_compose::error::ComposeError> {
        if let Some(b) = self.backend.get() {
            return Ok(Arc::clone(b));
        }

        let _guard = self.backend_init_lock.lock().await;

        if let Some(b) = self.backend.get() {
            return Ok(Arc::clone(b));
        }

        let backend = perry_container_compose::detect_backend().await?;
        let backend_arc: Arc<dyn ContainerBackend> = Arc::from(backend);
        let _ = self.backend.set(Arc::clone(&backend_arc));
        Ok(backend_arc)
    }
}
