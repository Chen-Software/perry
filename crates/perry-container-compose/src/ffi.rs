//! FFI exports for Perry TypeScript integration.

use crate::compose::{Orchestrator, ContainerStatus, ServiceStatus, ExecResult};
use crate::error::Result;
use std::path::PathBuf;
use std::collections::HashMap;

/// Wire layout of a Perry JS string header (matches perry-runtime)
#[repr(C)]
pub struct StringHeader {
    pub length: u32,
}

unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let len = (*ptr).length as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    Some(String::from_utf8_lossy(bytes).into_owned())
}

fn json_ok(value: &str) -> *const StringHeader {
    let payload = format!("{{\"ok\":true,\"result\":{}}}", value);
    heap_string(payload)
}

fn json_err(message: &str) -> *const StringHeader {
    let escaped = message.replace('"', "\\\"");
    let payload = format!("{{\"ok\":false,\"error\":\"{}\"}}", escaped);
    heap_string(payload)
}

fn heap_string(s: String) -> *const StringHeader {
    let bytes = s.into_bytes();
    let total = std::mem::size_of::<StringHeader>() + bytes.len();
    let layout = std::alloc::Layout::from_size_align(total, std::mem::align_of::<StringHeader>())
        .expect("layout");
    unsafe {
        let ptr = std::alloc::alloc(layout) as *mut StringHeader;
        (*ptr).length = bytes.len() as u32;
        let data_ptr = (ptr as *mut u8).add(std::mem::size_of::<StringHeader>());
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), data_ptr, bytes.len());
        ptr as *const StringHeader
    }
}

fn block<F: std::future::Future<Output = T>, T>(fut: F) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime")
        .block_on(fut)
}

fn parse_compose_file(file_ptr: *const StringHeader) -> Option<PathBuf> {
    unsafe { string_from_header(file_ptr) }.map(PathBuf::from)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_up(spec_json_ptr: *const StringHeader) -> *const StringHeader {
    // In-process orchestration would normally use the library API directly,
    // but this exported FFI is used by the compiler for the perry/container-compose module.
    json_err("use perry/container-compose up instead")
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_start(file_ptr: *const StringHeader) -> *const StringHeader {
    let files: Vec<PathBuf> = parse_compose_file(file_ptr).into_iter().collect();

    match Orchestrator::new(&files, None, &[]) {
        Err(e) => json_err(&e.to_string()),
        Ok(o) => match block(o.up(&[], true, false)) {
            Ok(()) => json_ok("null"),
            Err(e) => json_err(&e.to_string()),
        },
    }
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_stop(file_ptr: *const StringHeader) -> *const StringHeader {
    let files: Vec<PathBuf> = parse_compose_file(file_ptr).into_iter().collect();

    match Orchestrator::new(&files, None, &[]) {
        Err(e) => json_err(&e.to_string()),
        Ok(o) => match block(o.down(&[], false, false)) {
            Ok(()) => json_ok("null"),
            Err(e) => json_err(&e.to_string()),
        },
    }
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_ps(file_ptr: *const StringHeader) -> *const StringHeader {
    let files: Vec<PathBuf> = parse_compose_file(file_ptr).into_iter().collect();

    match Orchestrator::new(&files, None, &[]) {
        Err(e) => json_err(&e.to_string()),
        Ok(o) => match block(o.ps()) {
            Err(e) => json_err(&e.to_string()),
            Ok(statuses) => {
                let items: Vec<String> = statuses
                    .iter()
                    .map(|s| {
                        let status_str = match s.status {
                            ContainerStatus::Running => "running",
                            ContainerStatus::Stopped => "stopped",
                            ContainerStatus::NotFound => "not_found",
                        };
                        format!(
                            "{{\"service\":\"{}\",\"container\":\"{}\",\"status\":\"{}\"}}",
                            s.service_name, s.container_name, status_str
                        )
                    })
                    .collect();
                let array = format!("[{}]", items.join(","));
                json_ok(&array)
            }
        },
    }
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_logs(
    file_ptr: *const StringHeader,
    services_ptr: *const StringHeader,
    follow: bool,
) -> *const StringHeader {
    let files: Vec<PathBuf> = parse_compose_file(file_ptr).into_iter().collect();
    let services: Vec<String> = string_from_header(services_ptr)
        .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
        .unwrap_or_default();

    match Orchestrator::new(&files, None, &[]) {
        Err(e) => json_err(&e.to_string()),
        Ok(o) => match block(o.logs(&services, None, follow)) {
            Err(e) => json_err(&e.to_string()),
            Ok(logs_map) => {
                let pairs: Vec<String> = logs_map
                    .iter()
                    .map(|(k, v)| {
                        let escaped = v.replace('"', "\\\"").replace('\n', "\\n");
                        format!("\"{}\":\"{}\"", k, escaped)
                    })
                    .collect();
                let obj = format!("{{{}}}", pairs.join(","));
                json_ok(&obj)
            }
        },
    }
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_exec(
    file_ptr: *const StringHeader,
    service_ptr: *const StringHeader,
    cmd_ptr: *const StringHeader,
) -> *const StringHeader {
    let files: Vec<PathBuf> = parse_compose_file(file_ptr).into_iter().collect();
    let service = match string_from_header(service_ptr) {
        Some(s) => s,
        None => return json_err("service name is required"),
    };
    let cmd: Vec<String> = string_from_header(cmd_ptr)
        .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
        .unwrap_or_default();

    match Orchestrator::new(&files, None, &[]) {
        Err(e) => json_err(&e.to_string()),
        Ok(o) => match block(o.exec(&service, &cmd, None, None, None)) {
            Err(e) => json_err(&e.to_string()),
            Ok(result) => {
                let stdout = result.stdout.replace('"', "\\\"").replace('\n', "\\n");
                let stderr = result.stderr.replace('"', "\\\"").replace('\n', "\\n");
                let payload = format!(
                    "{{\"stdout\":\"{}\",\"stderr\":\"{}\",\"exitCode\":{}}}",
                    stdout, stderr, result.exit_code
                );
                json_ok(&payload)
            }
        },
    }
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_config(file_ptr: *const StringHeader) -> *const StringHeader {
    let files: Vec<PathBuf> = parse_compose_file(file_ptr).into_iter().collect();

    match Orchestrator::new(&files, None, &[]) {
        Err(e) => json_err(&e.to_string()),
        Ok(o) => match o.config() {
            Err(e) => json_err(&e.to_string()),
            Ok(yaml) => {
                let escaped = yaml.replace('"', "\\\"").replace('\n', "\\n");
                json_ok(&format!("\"{}\"", escaped))
            }
        },
    }
}
