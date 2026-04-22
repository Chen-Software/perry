use std::sync::{Arc, OnceLock};
use dashmap::DashMap;
use crate::backend::{ContainerBackend, detect_backend};
use crate::types::ComposeError;
use super::compose::ComposeEngine;
use tokio::sync::Mutex;

pub enum HandleEntry {
    Container(crate::types::ContainerHandle),
    Compose(Arc<ComposeEngine>),
}

pub struct ContainerContext {
    backend: Mutex<Option<Arc<dyn ContainerBackend + Send + Sync>>>,
    pub handles: DashMap<u64, HandleEntry>,
}

static GLOBAL_CONTEXT: OnceLock<ContainerContext> = OnceLock::new();

impl ContainerContext {
    pub fn global() -> &'static ContainerContext {
        GLOBAL_CONTEXT.get_or_init(Self::new)
    }

    pub fn new() -> Self {
        Self {
            backend: Mutex::new(None),
            handles: DashMap::new(),
        }
    }

    pub async fn get_backend(&self) -> Result<Arc<dyn ContainerBackend + Send + Sync>, String> {
        let mut backend_lock = self.backend.lock().await;
        if let Some(b) = backend_lock.as_ref() {
            return Ok(Arc::clone(b));
        }

        match detect_backend().await {
            Ok(b) => {
                *backend_lock = Some(Arc::clone(&b));
                Ok(b)
            }
            Err(probed) => Err(format!("No container backend found. Probed: {:?}", probed)),
        }
    }
}

pub async fn get_global_backend_instance() -> Result<Arc<dyn ContainerBackend + Send + Sync>, String> {
    ContainerContext::global().get_backend().await
}
