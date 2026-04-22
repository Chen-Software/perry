//! Type re-exports for container module

pub use perry_container_compose::types::*;
pub use perry_container_compose::error::ComposeError;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use dashmap::DashMap;

// ============ Handle Management ============

static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);
pub static CONTAINER_HANDLES: OnceLock<DashMap<u64, ContainerHandle>> = OnceLock::new();
pub static COMPOSE_ENGINES: OnceLock<DashMap<u64, Arc<super::compose::ComposeEngine>>> = OnceLock::new();

pub fn register_container_handle(handle: ContainerHandle) -> u64 {
    let id = NEXT_HANDLE.fetch_add(1, Ordering::SeqCst);
    CONTAINER_HANDLES.get_or_init(DashMap::new).insert(id, handle);
    id
}

pub fn register_compose_engine(engine: super::compose::ComposeEngine) -> u64 {
    let id = NEXT_HANDLE.fetch_add(1, Ordering::SeqCst);
    COMPOSE_ENGINES.get_or_init(DashMap::new).insert(id, Arc::new(engine));
    id
}

pub fn register_compose_engine_arc(engine: Arc<super::compose::ComposeEngine>) -> u64 {
    let id = NEXT_HANDLE.fetch_add(1, Ordering::SeqCst);
    COMPOSE_ENGINES.get_or_init(DashMap::new).insert(id, engine);
    id
}
