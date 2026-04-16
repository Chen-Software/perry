//! Single-container types and handle registries.

pub use perry_container_compose::types::{
    BuildSpec, ComposeConfig, ComposeDependsOn, ComposeHealthcheck, ComposeLogging,
    ComposeNetwork, ComposeNetworkIpam, ComposeNetworkIpamConfig, ComposeResourceSpec,
    ComposeSecret, ComposeService, ComposeServiceBuild, ComposeServiceNetworkConfig,
    ComposeServicePort, ComposeServiceVolume, ComposeServiceVolumeBind,
    ComposeServiceVolumeImage, ComposeServiceVolumeOpts, ComposeServiceVolumeTmpfs,
    ComposeSpec, ContainerHandle, ContainerInfo, ContainerLogs, ContainerSpec,
    DependsOnCondition, DependsOnSpec, ImageInfo, ListOrDict, PortSpec, ServiceNetworks,
    VolumeType, ComposeHandle,
};

pub use perry_container_compose::error::ComposeError as ContainerError;

use perry_container_compose::ComposeEngine;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use dashmap::DashMap;

static CONTAINER_HANDLES: OnceLock<DashMap<u64, ContainerHandle>> = OnceLock::new();
static COMPOSE_HANDLES: OnceLock<DashMap<u64, ComposeEngine>> = OnceLock::new();
static NEXT_HANDLE_ID: AtomicU64 = AtomicU64::new(1);

pub fn register_container_handle(handle: ContainerHandle) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    CONTAINER_HANDLES.get_or_init(DashMap::new).insert(id, handle);
    id
}

pub fn take_container_handle(id: u64) -> Option<ContainerHandle> {
    CONTAINER_HANDLES.get_or_init(DashMap::new).remove(&id).map(|(_, h)| h)
}

pub fn register_compose_handle(engine: ComposeEngine) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    COMPOSE_HANDLES.get_or_init(DashMap::new).insert(id, engine);
    id
}

pub fn get_compose_handle(id: u64) -> Option<dashmap::mapref::one::Ref<'static, u64, ComposeEngine>> {
    COMPOSE_HANDLES.get_or_init(DashMap::new).get(&id)
}

pub fn take_compose_handle(id: u64) -> Option<ComposeEngine> {
    COMPOSE_HANDLES.get_or_init(DashMap::new).remove(&id).map(|(_, e)| e)
}

// FFI Helpers
pub fn register_container_info_list(list: Vec<ContainerInfo>) -> u64 {
    let _ = list;
    0
}

pub fn register_container_info(info: ContainerInfo) -> u64 {
    let _ = info;
    0
}

pub fn register_container_logs(logs: ContainerLogs) -> u64 {
    let _ = logs;
    0
}

pub fn register_image_info_list(list: Vec<ImageInfo>) -> u64 {
    let _ = list;
    0
}
