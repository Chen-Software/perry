//! Type definitions for the perry/container module.

use perry_runtime::StringHeader;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use dashmap::DashMap;

use perry_container_compose::ComposeEngine;

// ============ Handle Registry ============

pub use perry_container_compose::types::{
    ContainerHandle, ContainerInfo, ContainerLogs, ImageInfo, ComposeHandle, ContainerSpec, ComposeSpec, ListOrDict
};
use perry_container_compose::ComposeError;

#[derive(Debug, thiserror::Error)]
pub enum ContainerError {
    #[error("{message}")]
    BackendError { code: i32, message: String },
    #[error("No container backend found. Probed: {probed:?}")]
    NoBackendFound { probed: Vec<perry_container_compose::backend::BackendProbeResult> },
    #[error("Not found: {0}")]
    NotFound(String),
}

impl From<ComposeError> for ContainerError {
    fn from(e: ComposeError) -> Self {
        match e {
            ComposeError::BackendError { code, message } => ContainerError::BackendError { code, message },
            ComposeError::NoBackendFound { probed } => ContainerError::NoBackendFound { probed },
            ComposeError::NotFound(s) => ContainerError::NotFound(s),
            _ => ContainerError::BackendError { code: -1, message: e.to_string() },
        }
    }
}

impl From<String> for ContainerError {
    fn from(s: String) -> Self {
        ContainerError::BackendError { code: -1, message: s }
    }
}

pub static CONTAINER_HANDLES: OnceLock<DashMap<u64, ContainerHandle>> = OnceLock::new();
pub static COMPOSE_HANDLES: OnceLock<DashMap<u64, ArcComposeEngine>> = OnceLock::new();
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

pub fn get_compose_handle(id: u64) -> Option<ArcComposeEngine> {
    COMPOSE_HANDLES.get()?.get(&id).map(|e| ArcComposeEngine(std::sync::Arc::clone(&e.0)))
}

pub fn take_compose_handle(id: u64) -> Option<ArcComposeEngine> {
    COMPOSE_HANDLES.get()?.remove(&id).map(|(_, e)| e)
}

pub fn parse_container_spec(ptr: *const StringHeader) -> Result<ContainerSpec, String> {
    let json = unsafe { string_from_header(ptr) }.ok_or("Invalid JSON pointer")?;
    serde_json::from_str(&json).map_err(|e| e.to_string())
}

pub fn parse_compose_spec(ptr: *const StringHeader) -> Result<perry_container_compose::types::ComposeSpec, String> {
    let json = unsafe { string_from_header(ptr) }.ok_or("Invalid JSON pointer")?;
    serde_json::from_str(&json).map_err(|e| e.to_string())
}


// ============ Helper for StringHeader ============

pub unsafe fn string_from_header(header: *const StringHeader) -> Option<String> {
    if header.is_null() || (header as usize) < 0x1000 {
        return None;
    }
    let s = (*header).as_str();
    Some(s.to_string())
}
