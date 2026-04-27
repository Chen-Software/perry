use std::sync::Arc;
use tokio::sync::Mutex;
use perry_container_compose::backend::{ContainerBackend, get_global_backend_instance};
use perry_container_compose::error::ComposeError;
use dashmap::DashMap;

pub struct ContainerContext {
    pub backend: Mutex<Option<Arc<dyn ContainerBackend>>>,
    pub handles: DashMap<u64, HandleEntry>,
}

pub enum HandleEntry {
    Container(String),
    Compose(u64), // placeholder
}

impl ContainerContext {
    pub fn global() -> &'static Self {
        static INSTANCE: once_cell::sync::Lazy<ContainerContext> = once_cell::sync::Lazy::new(|| {
            ContainerContext {
                backend: Mutex::new(None),
                handles: DashMap::new(),
            }
        });
        &INSTANCE
    }

    pub async fn get_backend(&self) -> Result<Arc<dyn ContainerBackend>, ComposeError> {
        let mut lock = self.backend.lock().await;
        if let Some(b) = &*lock {
            return Ok(b.clone());
        }
        let b = get_global_backend_instance().await?;
        *lock = Some(b.clone());
        Ok(b)
    }
}
