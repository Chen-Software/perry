//! Type definitions for the perry/container module.

use perry_runtime::StringHeader;
pub use perry_container_compose::types::{
    ComposeHandle, ComposeService, ComposeSpec, ContainerHandle, ContainerInfo,
    ContainerLogs, ContainerSpec, ImageInfo,
};
pub use perry_container_compose::error::ComposeError as ContainerError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use perry_container_compose::ComposeEngine;
use crate::common::handle::{register_handle, take_handle, get_handle};

// ============ Handle Registry ============

pub fn register_container_handle(handle: ContainerHandle) -> u64 {
    register_handle(handle) as u64
}

pub fn register_compose_handle(engine: Arc<ComposeEngine>) -> u64 {
    register_handle(engine) as u64
}

pub fn get_compose_handle(id: u64) -> Option<&'static Arc<ComposeEngine>> {
    get_handle::<Arc<ComposeEngine>>(id as i64)
}

pub fn take_compose_handle(id: u64) -> Option<Arc<ComposeEngine>> {
    take_handle::<Arc<ComposeEngine>>(id as i64)
}

pub fn register_container_info_list(list: Vec<ContainerInfo>) -> u64 {
    register_handle(list) as u64
}

pub fn register_container_info(info: ContainerInfo) -> u64 {
    register_handle(info) as u64
}

pub fn register_container_logs(logs: ContainerLogs) -> u64 {
    register_handle(logs) as u64
}

pub fn register_image_info_list(list: Vec<ImageInfo>) -> u64 {
    register_handle(list) as u64
}


pub fn parse_container_spec(ptr: *const StringHeader) -> Result<ContainerSpec, String> {
    let s = unsafe { string_from_header(ptr) }.ok_or("Invalid StringHeader pointer")?;
    serde_json::from_str(&s).map_err(|e| format!("JSON error: {}", e))
}

pub fn parse_compose_spec(ptr: *const StringHeader) -> Result<ComposeSpec, String> {
    let s = unsafe { string_from_header(ptr) }.ok_or("Invalid StringHeader pointer")?;
    serde_json::from_str(&s).map_err(|e| format!("JSON error: {}", e))
}

// ============ Helper for StringHeader ============

pub unsafe fn string_from_header(header: *const StringHeader) -> Option<String> {
    if header.is_null() || (header as usize) < 0x1000 {
        return None;
    }
    let s = (*header).as_str();
    Some(s.to_string())
}
