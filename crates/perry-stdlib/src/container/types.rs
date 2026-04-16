use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use perry_runtime::StringHeader;

pub use perry_container_compose::types::{
    ComposeHandle, ComposeSpec, ListOrDict, ContainerSpec, ContainerHandle,
    ContainerInfo, ContainerLogs, ImageInfo
};
pub use perry_container_compose::error::ComposeError as ContainerError;

pub unsafe fn parse_container_spec(ptr: *const StringHeader) -> Result<ContainerSpec, String> {
    let s = string_from_header(ptr).ok_or("Invalid pointer")?;
    serde_json::from_str(&s).map_err(|e| e.to_string())
}

pub unsafe fn parse_compose_spec(ptr: *const StringHeader) -> Result<ComposeSpec, String> {
    let s = string_from_header(ptr).ok_or("Invalid pointer")?;
    serde_json::from_str(&s).map_err(|e| e.to_string())
}

unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() { return None; }
    let len = (*ptr).byte_len as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    Some(String::from_utf8_lossy(bytes).to_string())
}

static CONTAINER_HANDLES: once_cell::sync::Lazy<std::sync::Mutex<HashMap<u64, ContainerHandle>>> =
    once_cell::sync::Lazy::new(|| std::sync::Mutex::new(HashMap::new()));

static CONTAINER_INFO_LISTS: once_cell::sync::Lazy<std::sync::Mutex<HashMap<u64, Vec<ContainerInfo>>>> =
    once_cell::sync::Lazy::new(|| std::sync::Mutex::new(HashMap::new()));

static CONTAINER_LOGS: once_cell::sync::Lazy<std::sync::Mutex<HashMap<u64, ContainerLogs>>> =
    once_cell::sync::Lazy::new(|| std::sync::Mutex::new(HashMap::new()));

static CONTAINER_INFOS: once_cell::sync::Lazy<std::sync::Mutex<HashMap<u64, ContainerInfo>>> =
    once_cell::sync::Lazy::new(|| std::sync::Mutex::new(HashMap::new()));

static IMAGE_INFO_LISTS: once_cell::sync::Lazy<std::sync::Mutex<HashMap<u64, Vec<ImageInfo>>>> =
    once_cell::sync::Lazy::new(|| std::sync::Mutex::new(HashMap::new()));

static NEXT_HANDLE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn next_id() -> u64 {
    NEXT_HANDLE_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
}

pub fn register_container_handle(handle: ContainerHandle) -> u64 {
    let id = next_id();
    CONTAINER_HANDLES.lock().unwrap().insert(id, handle);
    id
}

pub fn register_container_info_list(infos: Vec<ContainerInfo>) -> u64 {
    let id = next_id();
    CONTAINER_INFO_LISTS.lock().unwrap().insert(id, infos);
    id
}

pub fn take_container_info_list(h: u64) -> Option<Vec<ContainerInfo>> {
    CONTAINER_INFO_LISTS.lock().unwrap().remove(&h)
}

pub fn register_container_info(info: ContainerInfo) -> u64 {
    let id = next_id();
    CONTAINER_INFOS.lock().unwrap().insert(id, info);
    id
}

pub fn take_container_info(h: u64) -> Option<ContainerInfo> {
    CONTAINER_INFOS.lock().unwrap().remove(&h)
}

pub fn register_container_logs(logs: ContainerLogs) -> u64 {
    let id = next_id();
    CONTAINER_LOGS.lock().unwrap().insert(id, logs);
    id
}

pub fn take_container_logs(h: u64) -> Option<ContainerLogs> {
    CONTAINER_LOGS.lock().unwrap().remove(&h)
}

pub fn register_image_info_list(images: Vec<ImageInfo>) -> u64 {
    let id = next_id();
    IMAGE_INFO_LISTS.lock().unwrap().insert(id, images);
    id
}

pub fn take_image_info_list(h: u64) -> Option<Vec<ImageInfo>> {
    IMAGE_INFO_LISTS.lock().unwrap().remove(&h)
}
pub fn register_compose_handle(handle: ComposeHandle) -> u64 { handle.stack_id }

pub fn take_compose_handle(id: u64) -> Option<ComposeHandle> {
    Some(ComposeHandle { stack_id: id, project_name: "".into(), services: vec![] })
}
pub fn get_compose_handle(id: u64) -> Option<ComposeHandle> {
    Some(ComposeHandle { stack_id: id, project_name: "".into(), services: vec![] })
}
