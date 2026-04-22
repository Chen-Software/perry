//! Standalone FFI entry points for perry-container-compose.
//!
//! Provides `js_container_compose_*` symbols for linking scenarios
//! where `perry-stdlib` is not present.

use crate::compose::{self, ComposeEngine};
use crate::error::compose_error_to_js;
use crate::types::{ComposeSpec, ContainerLogs};
use std::sync::Arc;

fn block<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Handle::current().block_on(f)
}

fn json_err(msg: &str) -> String {
    serde_json::json!({ "message": msg, "code": 500 }).to_string()
}

#[no_mangle]
pub extern "C" fn js_container_compose_up(spec_json: *const i8) -> u64 {
    let spec_str = unsafe { std::ffi::CStr::from_ptr(spec_json) }.to_str().unwrap_or("{}");
    let spec: ComposeSpec = match serde_json::from_str(spec_str) {
        Ok(s) => s,
        Err(_) => return 0,
    };

    let res = block(async move {
        let backend = crate::backend::detect_backend().await.map_err(|_| "No backend found")?;
        let engine = ComposeEngine::new(spec, "perry-stack".into(), Arc::from(backend));
        engine.up(&[], true, false, false).await.map_err(|e| e.to_string())
    });

    match res {
        Ok(h) => h.stack_id,
        Err(_) => 0,
    }
}

#[no_mangle]
pub extern "C" fn js_container_compose_down(handle_id: u64, volumes: i32) -> i32 {
    let engine = match ComposeEngine::get_engine(handle_id) {
        Some(e) => e,
        None => return -1,
    };
    match block(engine.down(&[], false, volumes != 0)) {
        Ok(_) => 0,
        Err(_) => -1,
    }
}

#[no_mangle]
pub extern "C" fn js_container_compose_ps(handle_id: u64) -> *mut i8 {
    let engine = match ComposeEngine::get_engine(handle_id) {
        Some(e) => e,
        None => return std::ptr::null_mut(),
    };
    match block(engine.ps()) {
        Ok(list) => {
            let json = serde_json::to_string(&list).unwrap_or_default();
            std::ffi::CString::new(json).unwrap().into_raw()
        }
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn js_container_compose_logs(handle_id: u64, service: *const i8, tail: i32) -> *mut i8 {
    let engine = match ComposeEngine::get_engine(handle_id) {
        Some(e) => e,
        None => return std::ptr::null_mut(),
    };
    let svc_str = if service.is_null() { None } else { unsafe { std::ffi::CStr::from_ptr(service) }.to_str().ok() };
    let svcs = svc_str.map(|s| vec![s.to_string()]).unwrap_or_default();
    let t = if tail < 0 { None } else { Some(tail as u32) };

    match block(engine.logs(&svcs, t)) {
        Ok(logs_map) => {
            let mut stdout = String::new();
            let mut stderr = String::new();
            for (name, logs) in logs_map {
                stdout.push_str(&format!("--- {} ---\n{}", name, logs.stdout));
                stderr.push_str(&format!("--- {} ---\n{}", name, logs.stderr));
            }
            let json = serde_json::to_string(&ContainerLogs { stdout, stderr }).unwrap_or_default();
            std::ffi::CString::new(json).unwrap().into_raw()
        }
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn js_container_compose_exec(handle_id: u64, service: *const i8, cmd_json: *const i8) -> *mut i8 {
    let engine = match ComposeEngine::get_engine(handle_id) {
        Some(e) => e,
        None => return std::ptr::null_mut(),
    };
    let svc = unsafe { std::ffi::CStr::from_ptr(service) }.to_str().unwrap_or_default();
    let cmd_str = unsafe { std::ffi::CStr::from_ptr(cmd_json) }.to_str().unwrap_or("[]");
    let cmd: Vec<String> = serde_json::from_str(cmd_str).unwrap_or_default();

    match block(engine.exec(svc, &cmd)) {
        Ok(logs) => {
            let json = serde_json::to_string(&logs).unwrap_or_default();
            std::ffi::CString::new(json).unwrap().into_raw()
        }
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn js_container_compose_config(spec_json: *const i8) -> *mut i8 {
    let spec_str = unsafe { std::ffi::CStr::from_ptr(spec_json) }.to_str().unwrap_or("{}");
    let spec: ComposeSpec = match serde_json::from_str(spec_str) {
        Ok(s) => s,
        Err(e) => return std::ffi::CString::new(json_err(&e.to_string())).unwrap().into_raw(),
    };
    let res = block(async move {
        let backend = crate::backend::detect_backend().await.map_err(|_| "No backend found")?;
        let engine = ComposeEngine::new(spec, "perry-stack".into(), Arc::from(backend));
        engine.config().map_err(|e| e.to_string())
    });
    match res {
        Ok(yaml) => std::ffi::CString::new(yaml).unwrap().into_raw(),
        Err(e) => std::ffi::CString::new(json_err(&e)).unwrap().into_raw(),
    }
}
