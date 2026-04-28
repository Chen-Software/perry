//! Type definitions for the perry/container module.

use perry_runtime::StringHeader;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use dashmap::DashMap;

pub use perry_container_compose::types::{
    ContainerInfo, ContainerLogs, ContainerSpec, ContainerHandle,
    ComposeServiceBuild, ImageInfo, WorkloadGraph, WorkloadNode,
    RunGraphOptions, StackStatus
};
use perry_container_compose::ComposeEngine;

// ============ Handle Registry ============

pub static CONTAINER_HANDLES: OnceLock<DashMap<u64, ContainerHandle>> = OnceLock::new();
pub static COMPOSE_HANDLES: OnceLock<DashMap<u64, ArcComposeEngine>> = OnceLock::new();
pub static CONTAINER_INFO_LIST_HANDLES: OnceLock<DashMap<u64, Vec<ContainerInfo>>> = OnceLock::new();
pub static CONTAINER_LOGS_HANDLES: OnceLock<DashMap<u64, ContainerLogs>> = OnceLock::new();
pub static WORKLOAD_GRAPH_HANDLES: OnceLock<DashMap<u64, WorkloadGraph>> = OnceLock::new();
pub static WORKLOAD_NODE_HANDLES: OnceLock<DashMap<u64, WorkloadNode>> = OnceLock::new();
pub static NEXT_HANDLE_ID: AtomicU64 = AtomicU64::new(1);

pub struct ArcComposeEngine(pub std::sync::Arc<ComposeEngine>);

pub fn register_container_handle(handle: ContainerHandle) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    CONTAINER_HANDLES.get_or_init(DashMap::new).insert(id, handle);
    id
}

pub fn register_compose_handle(engine: ComposeEngine) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    COMPOSE_HANDLES.get_or_init(DashMap::new).insert(id, ArcComposeEngine(std::sync::Arc::new(engine)));
    id
}

// ============ Helper for StringHeader ============

pub unsafe fn string_from_header(header: *const StringHeader) -> Option<String> {
    if header.is_null() || (header as usize) < 0x1000 {
        return None;
    }
    let s = (*header).as_str();
    Some(s.to_string())
}
