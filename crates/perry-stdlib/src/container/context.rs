use std::sync::{Arc, OnceLock};
use dashmap::DashMap;
use perry_container_compose::backend::ContainerBackend;
use crate::container::types::{ContainerHandle, ArcComposeEngine, ContainerError};
use tokio::sync::Mutex;

pub enum HandleEntry {
    Container(ContainerHandle),
    Compose(ArcComposeEngine),
}

pub struct ContainerContext {
    pub backend: Arc<OnceLock<Arc<dyn ContainerBackend>>>,
    pub backend_mutex: Arc<Mutex<()>>,
    pub handles: DashMap<u64, HandleEntry>,
}

impl ContainerContext {
    pub fn new() -> Self {
        Self {
            backend: Arc::new(OnceLock::new()),
            backend_mutex: Arc::new(Mutex::new(())),
            handles: DashMap::new(),
        }
    }

    pub fn global() -> &'static Self {
        static GLOBAL: OnceLock<ContainerContext> = OnceLock::new();
        GLOBAL.get_or_init(Self::new)
    }

    pub fn set_backend(&self, backend: Arc<dyn ContainerBackend>) {
        let _ = self.backend.set(backend);
    }

    pub async fn get_backend(&self) -> Result<Arc<dyn ContainerBackend>, ContainerError> {
        if let Some(backend) = self.backend.get() {
            return Ok(Arc::clone(backend));
        }

        let _guard = self.backend_mutex.lock().await;
        if let Some(backend) = self.backend.get() {
            return Ok(Arc::clone(backend));
        }

        let backend = perry_container_compose::backend::detect_backend()
            .await
            .map(Arc::from)
            .map_err(|e| ContainerError::from(e))?;

        let _ = self.backend.set(Arc::clone(&backend));
        Ok(backend)
    }
}
