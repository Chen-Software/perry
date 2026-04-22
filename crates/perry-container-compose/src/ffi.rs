//! FFI exports for Perry TypeScript integration.
//!
//! Each function follows the Perry FFI convention:
//! - String arguments arrive as `*const StringHeader` (Perry runtime layout)
//! - Results are serialised to JSON strings before being handed back to JS

use crate::compose::ComposeEngine;
use std::sync::Arc;

// ──────────────────────────────────────────────────────────────
// Minimal re-implementation of the Perry runtime string types
// ──────────────────────────────────────────────────────────────

#[repr(C)]
pub struct StringHeader {
    pub length: u32,
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
// Helpers
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

fn block<F: std::future::Future<Output = T>, T>(fut: F) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime")
        .block_on(fut)
}


// ──────────────────────────────────────────────────────────────
// Exported FFI functions
// ──────────────────────────────────────────────────────────────

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_up(spec_json: *const StringHeader) -> *const StringHeader {
    let spec_str = match string_from_header(spec_json) {
        Some(s) => s,
        None => return json_err("missing spec JSON"),
    };
    let spec: crate::types::ComposeSpec = match serde_json::from_str(&spec_str) {
        Ok(s) => s,
        Err(e) => return json_err(&format!("invalid ComposeSpec: {}", e)),
    };
    let backend = match block(crate::backend::detect_backend()) {
        Ok(b) => b,
        Err(e) => return json_err(&format!("no backend found: {:?}", e)),
    };
    let project_name = spec.name.clone().unwrap_or_else(|| "perry-stack".to_string());
    let engine = Arc::new(ComposeEngine::new(spec, project_name, backend));
    match block(engine.up(&[], true, false, false)) {
        Ok(handle) => json_ok(&serde_json::to_string(&handle).unwrap_or_default()),
        Err(e) => json_err(&e.to_string()),
    }
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_down(handle_id: u64, volumes: bool) -> *const StringHeader {
    match crate::compose::get_compose_engine(handle_id) {
        None => json_err("invalid stack handle"),
        Some(engine) => match block(engine.down(&[], false, volumes)) {
            Ok(_) => json_ok("null"),
            Err(e) => json_err(&e.to_string()),
        },
    }
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_ps(handle_id: u64) -> *const StringHeader {
    match crate::compose::get_compose_engine(handle_id) {
        None => json_err("invalid stack handle"),
        Some(engine) => match block(engine.ps()) {
            Err(e) => json_err(&e.to_string()),
            Ok(infos) => json_ok(&serde_json::to_string(&infos).unwrap_or_default()),
        },
    }
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_logs(
    handle_id: u64,
    services_ptr: *const StringHeader,
    tail: f64,
) -> *const StringHeader {
    let services: Vec<String> = string_from_header(services_ptr)
        .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
        .unwrap_or_default();
    let t = if tail < 0.0 { None } else { Some(tail as u32) };

    match crate::compose::get_compose_engine(handle_id) {
        None => json_err("invalid stack handle"),
        Some(engine) => match block(engine.logs(&services, t)) {
            Err(e) => json_err(&e.to_string()),
            Ok(logs) => {
                let combined = logs.values().cloned().collect::<Vec<_>>().join("\n");
                let payload = serde_json::json!({ "stdout": combined, "stderr": "" }).to_string();
                json_ok(&payload)
            }
        },
    }
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_exec(
    handle_id: u64,
    service_ptr: *const StringHeader,
    cmd_ptr: *const StringHeader,
) -> *const StringHeader {
    let service = match string_from_header(service_ptr) {
        Some(s) => s,
        None => return json_err("service name is required"),
    };
    let cmd: Vec<String> = string_from_header(cmd_ptr)
        .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
        .unwrap_or_default();

    match crate::compose::get_compose_engine(handle_id) {
        None => json_err("invalid stack handle"),
        Some(engine) => match block(engine.exec(&service, &cmd, None, None)) {
            Err(e) => json_err(&e.to_string()),
            Ok(result) => json_ok(&serde_json::to_string(&result).unwrap_or_default()),
        },
    }
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_config(handle_id: u64) -> *const StringHeader {
    match crate::compose::get_compose_engine(handle_id) {
        None => json_err("invalid stack handle"),
        Some(engine) => match engine.config() {
            Err(e) => json_err(&e.to_string()),
            Ok(yaml) => json_ok(&serde_json::to_string(&yaml).unwrap_or_default()),
        },
    }
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_start(handle_id: u64, services_ptr: *const StringHeader) -> *const StringHeader {
    let services: Vec<String> = string_from_header(services_ptr)
        .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
        .unwrap_or_default();
    match crate::compose::get_compose_engine(handle_id) {
        None => json_err("invalid stack handle"),
        Some(engine) => match block(engine.start(&services)) {
            Ok(_) => json_ok("null"),
            Err(e) => json_err(&e.to_string()),
        },
    }
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_stop(handle_id: u64, services_ptr: *const StringHeader) -> *const StringHeader {
    let services: Vec<String> = string_from_header(services_ptr)
        .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
        .unwrap_or_default();
    match crate::compose::get_compose_engine(handle_id) {
        None => json_err("invalid stack handle"),
        Some(engine) => match block(engine.stop(&services)) {
            Ok(_) => json_ok("null"),
            Err(e) => json_err(&e.to_string()),
        },
    }
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_restart(handle_id: u64, services_ptr: *const StringHeader) -> *const StringHeader {
    let services: Vec<String> = string_from_header(services_ptr)
        .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
        .unwrap_or_default();
    match crate::compose::get_compose_engine(handle_id) {
        None => json_err("invalid stack handle"),
        Some(engine) => match block(engine.restart(&services)) {
            Ok(_) => json_ok("null"),
            Err(e) => json_err(&e.to_string()),
        },
    }
}
