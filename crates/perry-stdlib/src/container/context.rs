use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;
use crate::common::handle::HandleEntry;
use dashmap::DashMap;
use perry_container_compose::backend::{ContainerBackend, get_global_backend_instance};
use perry_container_compose::error::ComposeError;

pub struct ContainerContext {
    pub backend: OnceLock<Arc<dyn ContainerBackend>>,
    pub handles: DashMap<u64, HandleEntry>,
}

impl ContainerContext {
    pub fn global() -> &'static Self {
        static INSTANCE: OnceLock<ContainerContext> = OnceLock::new();
        INSTANCE.get_or_init(|| Self {
            backend: OnceLock::new(),
            handles: DashMap::new(),
        })
    }

    pub async fn get_backend(&self) -> Result<Arc<dyn ContainerBackend>, ComposeError> {
        if let Some(b) = self.backend.get() {
            return Ok(b.clone());
        }

        let b = get_global_backend_instance().await?;
        let _ = self.backend.set(b.clone());
        Ok(b)
    }
}
