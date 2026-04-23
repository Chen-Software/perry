use std::sync::{Arc, OnceLock};
use dashmap::DashMap;
use crate::backend::{ContainerBackend, detect_backend};
use crate::types::ComposeError;
use super::compose::ComposeEngine;
use tokio::sync::OnceCell;

pub enum HandleEntry {
    Container(crate::types::ContainerHandle),
    Compose(Arc<ComposeEngine>),
}

pub struct ContainerContext {
    backend: OnceCell<Arc<dyn ContainerBackend + Send + Sync>>,
    pub handles: DashMap<u64, HandleEntry>,
}

static GLOBAL_CONTEXT: OnceLock<ContainerContext> = OnceLock::new();

impl ContainerContext {
    pub fn global() -> &'static ContainerContext {
        GLOBAL_CONTEXT.get_or_init(Self::new)
    }

    pub fn new() -> Self {
        Self {
            backend: OnceCell::new(),
            handles: DashMap::new(),
        }
    }

    pub fn get_backend_sync(&self) -> Option<Arc<dyn ContainerBackend + Send + Sync>> {
        self.backend.get().cloned()
    }

    pub async fn get_backend(&self) -> Result<Arc<dyn ContainerBackend + Send + Sync>, String> {
        self.backend.get_or_try_init(|| async {
            match detect_backend().await {
                Ok(b) => Ok(b),
                Err(probed) => {
                    let installer = perry_container_compose::installer::BackendInstaller { probed };
                    installer.run().await.map_err(|e| e.to_string())
                }
            }
        }).await.cloned()
    }
}

pub async fn get_global_backend_instance() -> Result<Arc<dyn ContainerBackend + Send + Sync>, String> {
    ContainerContext::global().get_backend().await
}
