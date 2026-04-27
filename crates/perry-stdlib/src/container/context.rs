//! Scoped state for container management.

use std::sync::{Arc, OnceLock};
use perry_container_compose::backend::ContainerBackend;
use dashmap::DashMap;
use tokio::sync::Mutex;
use crate::container::types::HandleEntry;

pub struct ContainerContext {
    pub backend: OnceLock<Arc<dyn ContainerBackend + Send + Sync>>,
    pub handles: DashMap<u64, HandleEntry>,
    pub(crate) init_lock: Mutex<()>,
}

impl ContainerContext {
    pub fn new() -> Self {
        Self {
            backend: OnceLock::new(),
            handles: DashMap::new(),
            init_lock: Mutex::const_new(()),
        }
    }

    pub fn global() -> &'static Self {
        static GLOBAL: OnceLock<ContainerContext> = OnceLock::new();
        GLOBAL.get_or_init(Self::new)
    }
}
