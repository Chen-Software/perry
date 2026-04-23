//! Type re-exports for container module

use dashmap::DashMap;
pub use perry_container_compose::types::*;
pub use perry_container_compose::error::ComposeError;
pub use perry_container_compose::ComposeEngine;
pub use perry_container_compose::workload::{
    WorkloadGraph, WorkloadNode, RunGraphOptions, GraphStatus, NodeInfo, NodeState, WorkloadEdge, WorkloadEnvValue,
    RuntimeSpec, PolicySpec, WorkloadRef, PolicyTier, RefProjection, WorkloadResources, ExecutionStrategy, FailureStrategy,
    WorkloadGraphEngine
};

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

// ============ Handle Management ============

pub static CONTAINER_HANDLES: OnceLock<DashMap<u64, ContainerHandle>> = OnceLock::new();
pub static COMPOSE_ENGINES: OnceLock<DashMap<u64, ComposeEngine>> = OnceLock::new();
pub static WORKLOAD_ENGINES: OnceLock<DashMap<u64, WorkloadGraphEngine>> = OnceLock::new();

pub static CONTAINER_INFOS: OnceLock<DashMap<u64, ContainerInfo>> = OnceLock::new();
pub static CONTAINER_INFO_LISTS: OnceLock<DashMap<u64, Vec<ContainerInfo>>> = OnceLock::new();
pub static WORKLOAD_NODE_INFO_LISTS: OnceLock<DashMap<u64, Vec<NodeInfo>>> = OnceLock::new();
pub static CONTAINER_LOGS_MAP: OnceLock<DashMap<u64, ContainerLogs>> = OnceLock::new();
pub static IMAGE_INFO_LISTS: OnceLock<DashMap<u64, Vec<ImageInfo>>> = OnceLock::new();

static NEXT_HANDLE_ID: AtomicU64 = AtomicU64::new(1);

fn next_id() -> u64 {
    NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst)
}

pub fn register_container_handle(handle: ContainerHandle) -> u64 {
    let id = next_id();
    CONTAINER_HANDLES.get_or_init(DashMap::new).insert(id, handle);
    id
}

pub fn register_compose_handle(engine: ComposeEngine) -> u64 {
    let id = next_id();
    COMPOSE_ENGINES.get_or_init(DashMap::new).insert(id, engine);
    id
}

pub fn register_workload_handle(engine: WorkloadGraphEngine) -> u64 {
    let id = next_id();
    WORKLOAD_ENGINES.get_or_init(DashMap::new).insert(id, engine);
    id
}

pub fn register_container_info(info: ContainerInfo) -> u64 {
    let id = next_id();
    CONTAINER_INFOS.get_or_init(DashMap::new).insert(id, info);
    id
}

pub fn register_container_info_list(list: Vec<ContainerInfo>) -> u64 {
    let id = next_id();
    CONTAINER_INFO_LISTS.get_or_init(DashMap::new).insert(id, list);
    id
}

pub fn register_workload_node_info_list(list: Vec<NodeInfo>) -> u64 {
    let id = next_id();
    WORKLOAD_NODE_INFO_LISTS.get_or_init(DashMap::new).insert(id, list);
    id
}

pub fn register_container_logs(logs: ContainerLogs) -> u64 {
    let id = next_id();
    CONTAINER_LOGS_MAP.get_or_init(DashMap::new).insert(id, logs);
    id
}

pub fn register_image_info_list(list: Vec<ImageInfo>) -> u64 {
    let id = next_id();
    IMAGE_INFO_LISTS.get_or_init(DashMap::new).insert(id, list);
    id
}
