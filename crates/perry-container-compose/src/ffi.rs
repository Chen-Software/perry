//! FFI exports for Perry TypeScript integration.
//!
//! Each function follows the Perry FFI convention:
//! - String arguments arrive as `*const StringHeader` (Perry runtime layout)
//! - Async operations return `*mut Promise` which is resolved/rejected on the tokio runtime
//! - Results are serialised to JSON strings before being handed back to JS

use crate::orchestrate::Orchestrator;
use std::collections::HashMap;
use std::path::PathBuf;

// ──────────────────────────────────────────────────────────────
// Minimal re-implementation of the Perry runtime string types
// so this crate does not have to depend on perry-runtime.
// In a real integration the compiler would link against perry-runtime
// and these types would come from there.
// ──────────────────────────────────────────────────────────────

/// Wire layout of a Perry JS string header (matches perry-runtime)
#[repr(C)]
pub struct StringHeader {
    pub length: u32,
    // Followed immediately in memory by `length` UTF-8 bytes
}

unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() || (ptr as usize) < 0x1000 {
        return None;
    }
    let len = (*ptr).length as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    Some(String::from_utf8_lossy(bytes).into_owned())
}

// ──────────────────────────────────────────────────────────────
// Helpers to build OwnedString replies.
// In production this would call perry_runtime::js_string_from_bytes.
// ──────────────────────────────────────────────────────────────

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

// ──────────────────────────────────────────────────────────────
// Synchronous wrappers — run tokio::block_on internally.
// Perry will expose these as async functions via generated Promise wrappers.
// ──────────────────────────────────────────────────────────────

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

// ──────────────────────────────────────────────────────────────
// Exported FFI functions
// ──────────────────────────────────────────────────────────────

/// `js_compose_start(file)` → JSON result
#[no_mangle]
pub unsafe extern "C" fn js_compose_start(file_ptr: *const StringHeader) -> *const StringHeader {
    let files: Vec<PathBuf> = parse_compose_file(file_ptr).into_iter().collect();

    match Orchestrator::new(&files, None, &[]) {
        Err(e) => json_err(&e.to_string()),
        Ok(o) => match block(o.up(&[], true, false)) {
            Ok(()) => json_ok("null"),
            Err(e) => json_err(&e.to_string()),
        },
    }
}

/// `js_compose_stop(file)` → JSON result
#[no_mangle]
pub unsafe extern "C" fn js_compose_stop(file_ptr: *const StringHeader) -> *const StringHeader {
    let files: Vec<PathBuf> = parse_compose_file(file_ptr).into_iter().collect();

    match Orchestrator::new(&files, None, &[]) {
        Err(e) => json_err(&e.to_string()),
        Ok(o) => match block(o.down(&[], false, false)) {
            Ok(()) => json_ok("null"),
            Err(e) => json_err(&e.to_string()),
        },
    }
}

/// `js_compose_ps(file)` → JSON result with ServiceStatus array
#[no_mangle]
pub unsafe extern "C" fn js_compose_ps(file_ptr: *const StringHeader) -> *const StringHeader {
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
                            crate::commands::ContainerStatus::Running => "running",
                            crate::commands::ContainerStatus::Stopped => "stopped",
                            crate::commands::ContainerStatus::NotFound => "not_found",
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

/// `js_compose_logs(file, services_json, follow)` → JSON result
#[no_mangle]
pub unsafe extern "C" fn js_compose_logs(
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

/// `js_compose_exec(file, service, cmd_json)` → JSON result
#[no_mangle]
pub unsafe extern "C" fn js_compose_exec(
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

/// `js_compose_config(file)` → JSON result with YAML string
#[no_mangle]
pub unsafe extern "C" fn js_compose_config(file_ptr: *const StringHeader) -> *const StringHeader {
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
