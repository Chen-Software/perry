//! Type re-exports and handle management for container module

pub use perry_container_compose::types::*;
pub use perry_container_compose::error::{ComposeError, Result};
pub use perry_container_compose::ComposeEngine;

use serde::{Deserialize, Serialize};
use perry_runtime::JSValue;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{OnceLock};
use dashmap::DashMap;

// ============ Global Handle Registries ============

pub static CONTAINER_HANDLES: OnceLock<DashMap<u64, ContainerHandle>> = OnceLock::new();
pub static COMPOSE_HANDLES: OnceLock<DashMap<u64, std::sync::Arc<ComposeEngine>>> = OnceLock::new();

pub static CONTAINER_INFOS: OnceLock<DashMap<u64, ContainerInfo>> = OnceLock::new();
pub static CONTAINER_INFO_LISTS: OnceLock<DashMap<u64, Vec<ContainerInfo>>> = OnceLock::new();
pub static CONTAINER_LOGS: OnceLock<DashMap<u64, ContainerLogs>> = OnceLock::new();
pub static IMAGE_INFO_LISTS: OnceLock<DashMap<u64, Vec<ImageInfo>>> = OnceLock::new();

static NEXT_HANDLE_ID: AtomicU64 = AtomicU64::new(1);

fn container_map() -> &'static DashMap<u64, ContainerHandle> {
    CONTAINER_HANDLES.get_or_init(DashMap::new)
}

fn compose_map() -> &'static DashMap<u64, std::sync::Arc<ComposeEngine>> {
    COMPOSE_HANDLES.get_or_init(DashMap::new)
}

// ============ Handle Management ============

pub fn register_container_handle(handle: ContainerHandle) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    container_map().insert(id, handle);
    id
}

pub fn register_compose_handle(engine: std::sync::Arc<ComposeEngine>) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    compose_map().insert(id, engine);
    id
}

pub fn register_container_info(info: ContainerInfo) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    CONTAINER_INFOS.get_or_init(DashMap::new).insert(id, info);
    id
}

pub fn register_container_info_list(list: Vec<ContainerInfo>) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    CONTAINER_INFO_LISTS.get_or_init(DashMap::new).insert(id, list);
    id
}

pub fn take_container_info_list(id: u64) -> Option<Vec<ContainerInfo>> {
    CONTAINER_INFO_LISTS.get()?.remove(&id).map(|(_, v)| v)
}

pub fn register_container_logs(logs: ContainerLogs) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    CONTAINER_LOGS.get_or_init(DashMap::new).insert(id, logs);
    id
}

pub fn take_container_logs(id: u64) -> Option<ContainerLogs> {
    CONTAINER_LOGS.get()?.remove(&id).map(|(_, v)| v)
}

pub fn register_image_info_list(list: Vec<ImageInfo>) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    IMAGE_INFO_LISTS.get_or_init(DashMap::new).insert(id, list);
    id
}

// ============ Error Types ============

#[derive(Debug, Serialize, Deserialize)]
pub struct ContainerError {
    pub message: String,
    pub code: i32,
}

impl From<ComposeError> for ContainerError {
    fn from(err: ComposeError) -> Self {
        let val = err.to_js_json();
        ContainerError {
            message: val["message"].as_str().unwrap_or("Unknown error").to_string(),
            code: val["code"].as_i64().unwrap_or(500) as i32,
        }
    }
}

impl ContainerError {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| r#"{"message":"Unknown error","code":500}"#.into())
    }
}

// ============ JSValue Parsing Functions ============

pub fn parse_container_spec(json: &str) -> serde_json::Result<ContainerSpec> {
    serde_json::from_str(json)
}

pub fn parse_compose_spec(json: &str) -> serde_json::Result<ComposeSpec> {
    serde_json::from_str(json)
}
