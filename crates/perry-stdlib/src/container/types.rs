//! Type definitions for container module

pub use perry_container_compose::types::{
    ComposeConfig, ComposeDependsOn, ComposeDeployment, ComposeHandle, ComposeHealthcheck,
    ComposeLogging, ComposeNetwork, ComposeNetworkIpam, ComposeSecret, ComposeService,
    ComposeServiceBuild, ComposeServiceNetworkConfig, ComposeServicePort, ComposeServiceVolume,
    ComposeSpec, ComposeVolume, ContainerHandle, ContainerInfo, ContainerLogs, ContainerSpec,
    ImageInfo, ListOrDict,
};

use dashmap::DashMap;
use perry_container_compose::compose::ComposeEngine;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

// ============ Handle Management ============

static CONTAINER_HANDLES: OnceLock<DashMap<u64, Arc<ContainerHandle>>> = OnceLock::new();
static COMPOSE_HANDLES: OnceLock<DashMap<u64, Arc<ComposeEngine>>> = OnceLock::new();
static NEXT_HANDLE_ID: AtomicU64 = AtomicU64::new(1);

fn next_id() -> u64 {
    NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst)
}

pub fn register_container_handle(handle: ContainerHandle) -> u64 {
    let id = next_id();
    CONTAINER_HANDLES.get_or_init(DashMap::new).insert(id, Arc::new(handle));
    id
}

pub fn get_container_handle(id: u64) -> Option<Arc<ContainerHandle>> {
    CONTAINER_HANDLES.get()?.get(&id).map(|r| Arc::clone(r.value()))
}

pub fn register_compose_handle(engine: ComposeEngine) -> u64 {
    let id = next_id();
    COMPOSE_HANDLES.get_or_init(DashMap::new).insert(id, Arc::new(engine));
    id
}

pub fn get_compose_engine(id: u64) -> Option<Arc<ComposeEngine>> {
    COMPOSE_HANDLES.get()?.get(&id).map(|r| Arc::clone(r.value()))
}
