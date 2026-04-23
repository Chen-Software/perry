pub mod types;
pub mod backend;
pub mod compose;
pub mod verification;
pub mod capability;
pub mod error;

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use dashmap::DashMap;
use perry_runtime::{Promise, StringHeader};
use crate::container::backend::{ContainerBackend, detect_backend, BackendProbeResult};
use crate::container::types::*;
use crate::container::error::ContainerError;
use crate::container::compose::ComposeEngine;

static BACKEND: OnceLock<Arc<dyn ContainerBackend>> = OnceLock::new();
static CONTAINER_HANDLES: OnceLock<DashMap<u64, ContainerHandle>> = OnceLock::new();
static COMPOSE_HANDLES: OnceLock<DashMap<u64, Arc<ComposeEngine>>> = OnceLock::new();

/// Helper to extract string from StringHeader pointer
pub(crate) unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() || (ptr as usize) < 0x1000 {
        return None;
    }
    let len = (*ptr).byte_len as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    Some(String::from_utf8_lossy(bytes).to_string())
}

fn get_backend() -> Result<Arc<dyn ContainerBackend>, ContainerError> {
    BACKEND.get().cloned().ok_or_else(|| ContainerError::BackendNotAvailable {
        name: "unknown".to_string(),
        reason: "Backend not initialized".to_string()
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_module_init() {
    // Force backend detection
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        if let Ok(backend) = detect_backend().await {
            BACKEND.set(Arc::from(backend)).ok();
        }
    });
}

#[no_mangle]
pub unsafe extern "C" fn js_container_run(spec_json: *const StringHeader) -> *mut Promise {
    let spec_str = string_from_header(spec_json).unwrap_or_default();
    let spec: ContainerSpec = match serde_json::from_str(&spec_str) {
        Ok(s) => s,
        Err(e) => {
            let p = perry_runtime::js_promise_new();
            let err_msg = format!("JSON error: {}", e);
            let s_ptr = perry_runtime::js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
            perry_runtime::js_promise_reject(p, f64::from_bits(perry_runtime::JSValue::string_ptr(s_ptr).bits()));
            return p;
        }
    };
    let promise = perry_runtime::js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_backend().map_err(|e| e.to_string())?;
        let id = backend.run(&spec).await.map_err(|e| e.to_string())?;
        let res = serde_json::to_string(&ContainerHandle { id, name: spec.name }).unwrap();
        let s_ptr = perry_runtime::js_string_from_bytes(res.as_ptr(), res.len() as u32);
        Ok(perry_runtime::JSValue::string_ptr(s_ptr).bits())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_start(handle_id: f64, services_json: *const StringHeader) -> *mut Promise {
    let services_str = string_from_header(services_json).unwrap_or_default();
    let services: Vec<String> = serde_json::from_str(&services_str).unwrap_or_default();
    let promise = perry_runtime::js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = COMPOSE_HANDLES.get().and_then(|m| m.get(&(handle_id as u64))).map(|r| r.value().clone());
        if let Some(engine) = engine {
            engine.start(&services).await.map_err(|e| e.to_string())?;
            Ok(perry_runtime::JSValue::undefined().bits())
        } else {
            Err("Compose engine not found".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_stop(handle_id: f64, services_json: *const StringHeader) -> *mut Promise {
    let services_str = string_from_header(services_json).unwrap_or_default();
    let services: Vec<String> = serde_json::from_str(&services_str).unwrap_or_default();
    let promise = perry_runtime::js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = COMPOSE_HANDLES.get().and_then(|m| m.get(&(handle_id as u64))).map(|r| r.value().clone());
        if let Some(engine) = engine {
            engine.stop(&services).await.map_err(|e| e.to_string())?;
            Ok(perry_runtime::JSValue::undefined().bits())
        } else {
            Err("Compose engine not found".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_restart(handle_id: f64, services_json: *const StringHeader) -> *mut Promise {
    let services_str = string_from_header(services_json).unwrap_or_default();
    let services: Vec<String> = serde_json::from_str(&services_str).unwrap_or_default();
    let promise = perry_runtime::js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = COMPOSE_HANDLES.get().and_then(|m| m.get(&(handle_id as u64))).map(|r| r.value().clone());
        if let Some(engine) = engine {
            engine.restart(&services).await.map_err(|e| e.to_string())?;
            Ok(perry_runtime::JSValue::undefined().bits())
        } else {
            Err("Compose engine not found".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_create(spec_json: *const StringHeader) -> *mut Promise {
    let spec_str = string_from_header(spec_json).unwrap_or_default();
    let spec: ContainerSpec = match serde_json::from_str(&spec_str) {
        Ok(s) => s,
        Err(e) => {
            let p = perry_runtime::js_promise_new();
            let err_msg = format!("JSON error: {}", e);
            let s_ptr = perry_runtime::js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
            perry_runtime::js_promise_reject(p, f64::from_bits(perry_runtime::JSValue::string_ptr(s_ptr).bits()));
            return p;
        }
    };
    let promise = perry_runtime::js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_backend().map_err(|e| e.to_string())?;
        let id = backend.create(&spec).await.map_err(|e| e.to_string())?;
        let res = serde_json::to_string(&ContainerHandle { id, name: spec.name }).unwrap();
        let s_ptr = perry_runtime::js_string_from_bytes(res.as_ptr(), res.len() as u32);
        Ok(perry_runtime::JSValue::string_ptr(s_ptr).bits())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_start(id_json: *const StringHeader) -> *mut Promise {
    let id = string_from_header(id_json).unwrap_or_default();
    let promise = perry_runtime::js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_backend().map_err(|e| e.to_string())?;
        backend.start(&id).await.map_err(|e| e.to_string())?;
        Ok(perry_runtime::JSValue::undefined().bits())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_stop(id_json: *const StringHeader, timeout: f64) -> *mut Promise {
    let id = string_from_header(id_json).unwrap_or_default();
    let promise = perry_runtime::js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_backend().map_err(|e| e.to_string())?;
        let t = if timeout > 0.0 { Some(timeout as u32) } else { None };
        backend.stop(&id, t).await.map_err(|e| e.to_string())?;
        Ok(perry_runtime::JSValue::undefined().bits())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_remove(id_json: *const StringHeader, force: f64) -> *mut Promise {
    let id = string_from_header(id_json).unwrap_or_default();
    let promise = perry_runtime::js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_backend().map_err(|e| e.to_string())?;
        backend.remove(&id, force != 0.0).await.map_err(|e| e.to_string())?;
        Ok(perry_runtime::JSValue::undefined().bits())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_list(all: f64) -> *mut Promise {
    let promise = perry_runtime::js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_backend().map_err(|e| e.to_string())?;
        let list = backend.list(all != 0.0).await.map_err(|e| e.to_string())?;
        let res = serde_json::to_string(&list).unwrap();
        let s_ptr = perry_runtime::js_string_from_bytes(res.as_ptr(), res.len() as u32);
        Ok(perry_runtime::JSValue::string_ptr(s_ptr).bits())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_inspect(id_json: *const StringHeader) -> *mut Promise {
    let id = string_from_header(id_json).unwrap_or_default();
    let promise = perry_runtime::js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_backend().map_err(|e| e.to_string())?;
        let info = backend.inspect(&id).await.map_err(|e| e.to_string())?;
        let res = serde_json::to_string(&info).unwrap();
        let s_ptr = perry_runtime::js_string_from_bytes(res.as_ptr(), res.len() as u32);
        Ok(perry_runtime::JSValue::string_ptr(s_ptr).bits())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_logs(id_json: *const StringHeader, tail: f64) -> *mut Promise {
    let id = string_from_header(id_json).unwrap_or_default();
    let promise = perry_runtime::js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_backend().map_err(|e| e.to_string())?;
        let t = if tail > 0.0 { Some(tail as u32) } else { None };
        let logs = backend.logs(&id, t).await.map_err(|e| e.to_string())?;
        let res = serde_json::to_string(&logs).unwrap();
        let s_ptr = perry_runtime::js_string_from_bytes(res.as_ptr(), res.len() as u32);
        Ok(perry_runtime::JSValue::string_ptr(s_ptr).bits())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_exec(
    id_json: *const StringHeader,
    cmd_json: *const StringHeader,
    env_json: *const StringHeader,
    workdir_json: *const StringHeader
) -> *mut Promise {
    let id = string_from_header(id_json).unwrap_or_default();
    let cmd_str = string_from_header(cmd_json).unwrap_or_default();
    let env_str = string_from_header(env_json).unwrap_or_default();
    let workdir = string_from_header(workdir_json);

    let cmd: Vec<String> = serde_json::from_str(&cmd_str).unwrap_or_default();
    let env: Option<HashMap<String, String>> = serde_json::from_str(&env_str).ok();

    let promise = perry_runtime::js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_backend().map_err(|e| e.to_string())?;
        let logs = backend.exec(&id, &cmd, env, workdir).await.map_err(|e| e.to_string())?;
        let res = serde_json::to_string(&logs).unwrap();
        let s_ptr = perry_runtime::js_string_from_bytes(res.as_ptr(), res.len() as u32);
        Ok(perry_runtime::JSValue::string_ptr(s_ptr).bits())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_pullImage(ref_json: *const StringHeader) -> *mut Promise {
    let reference = string_from_header(ref_json).unwrap_or_default();
    let promise = perry_runtime::js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_backend().map_err(|e| e.to_string())?;
        backend.pull_image(&reference).await.map_err(|e| e.to_string())?;
        Ok(perry_runtime::JSValue::undefined().bits())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_listImages() -> *mut Promise {
    let promise = perry_runtime::js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_backend().map_err(|e| e.to_string())?;
        let images = backend.list_images().await.map_err(|e| e.to_string())?;
        let res = serde_json::to_string(&images).unwrap();
        let s_ptr = perry_runtime::js_string_from_bytes(res.as_ptr(), res.len() as u32);
        Ok(perry_runtime::JSValue::string_ptr(s_ptr).bits())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_removeImage(ref_json: *const StringHeader, force: f64) -> *mut Promise {
    let reference = string_from_header(ref_json).unwrap_or_default();
    let promise = perry_runtime::js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_backend().map_err(|e| e.to_string())?;
        backend.remove_image(&reference, force != 0.0).await.map_err(|e| e.to_string())?;
        Ok(perry_runtime::JSValue::undefined().bits())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_getBackend() -> *const StringHeader {
    let name = BACKEND.get().map(|b| b.name()).unwrap_or("none");
    perry_runtime::js_string_from_bytes(name.as_ptr(), name.len() as u32)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_detectBackend() -> *mut Promise {
    let promise = perry_runtime::js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let mut probed = Vec::new();
        // This is a bit tricky as detect_backend only returns the success one.
        // We might need a separate function in perry-container-compose to probe all.
        // For now, let's just return the current backend if detected.
        if let Ok(backend) = detect_backend().await {
            probed.push(BackendProbeResult {
                name: backend.name().to_string(),
                available: true,
                reason: None,
                version: None,
            });
        }
        let res = serde_json::to_string(&probed).unwrap();
        let s_ptr = perry_runtime::js_string_from_bytes(res.as_ptr(), res.len() as u32);
        Ok(perry_runtime::JSValue::string_ptr(s_ptr).bits())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_composeUp(spec_json: *const StringHeader) -> *mut Promise {
    let spec_str = string_from_header(spec_json).unwrap_or_default();
    let spec: perry_container_compose::types::ComposeSpec = match serde_json::from_str(&spec_str) {
        Ok(s) => s,
        Err(e) => {
            let p = perry_runtime::js_promise_new();
            let err_msg = format!("JSON error: {}", e);
            let s_ptr = perry_runtime::js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
            perry_runtime::js_promise_reject(p, f64::from_bits(perry_runtime::JSValue::string_ptr(s_ptr).bits()));
            return p;
        }
    };
    let promise = perry_runtime::js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_backend().map_err(|e| e.to_string())?;
        let project_name = spec.name.clone().unwrap_or_else(|| "perry-stack".to_string());
        let engine = ComposeEngine::new(spec, project_name, backend);
        let handle = engine.up(true, false, false).await.map_err(|e| e.to_string())?;

        COMPOSE_HANDLES.get_or_init(DashMap::new).insert(handle.stack_id, engine);

        let res = serde_json::to_string(&handle).unwrap();
        let s_ptr = perry_runtime::js_string_from_bytes(res.as_ptr(), res.len() as u32);
        Ok(perry_runtime::JSValue::string_ptr(s_ptr).bits())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_up(spec_json: *const StringHeader) -> *mut Promise {
    js_container_composeUp(spec_json)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_down(handle_id: f64, volumes: f64) -> *mut Promise {
    let promise = perry_runtime::js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = COMPOSE_HANDLES.get().and_then(|m| m.get(&(handle_id as u64))).map(|r| r.value().clone());
        if let Some(engine) = engine {
            engine.down(volumes != 0.0, false).await.map_err(|e| e.to_string())?;
            Ok(perry_runtime::JSValue::undefined().bits())
        } else {
            Err("Compose engine not found".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_ps(handle_id: f64) -> *mut Promise {
    let promise = perry_runtime::js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = COMPOSE_HANDLES.get().and_then(|m| m.get(&(handle_id as u64))).map(|r| r.value().clone());
        if let Some(engine) = engine {
            let list = engine.ps().await.map_err(|e| e.to_string())?;
            let res = serde_json::to_string(&list).unwrap();
            let s_ptr = perry_runtime::js_string_from_bytes(res.as_ptr(), res.len() as u32);
            Ok(perry_runtime::JSValue::string_ptr(s_ptr).bits())
        } else {
            Err("Compose engine not found".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_logs(handle_id: f64, service_json: *const StringHeader, tail: f64) -> *mut Promise {
    let service = string_from_header(service_json);
    let promise = perry_runtime::js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = COMPOSE_HANDLES.get().and_then(|m| m.get(&(handle_id as u64))).map(|r| r.value().clone());
        if let Some(engine) = engine {
            let t = if tail > 0.0 { Some(tail as u32) } else { None };
            let logs = engine.logs(service.as_deref(), t).await.map_err(|e| e.to_string())?;
            let res = serde_json::to_string(&logs).unwrap();
            let s_ptr = perry_runtime::js_string_from_bytes(res.as_ptr(), res.len() as u32);
            Ok(perry_runtime::JSValue::string_ptr(s_ptr).bits())
        } else {
            Err("Compose engine not found".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_exec(handle_id: f64, service_json: *const StringHeader, cmd_json: *const StringHeader) -> *mut Promise {
    let service = string_from_header(service_json).unwrap_or_default();
    let cmd_str = string_from_header(cmd_json).unwrap_or_default();
    let cmd: Vec<String> = serde_json::from_str(&cmd_str).unwrap_or_default();

    let promise = perry_runtime::js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = COMPOSE_HANDLES.get().and_then(|m| m.get(&(handle_id as u64))).map(|r| r.value().clone());
        if let Some(engine) = engine {
            let logs = engine.exec(&service, &cmd).await.map_err(|e| e.to_string())?;
            let res = serde_json::to_string(&logs).unwrap();
            let s_ptr = perry_runtime::js_string_from_bytes(res.as_ptr(), res.len() as u32);
            Ok(perry_runtime::JSValue::string_ptr(s_ptr).bits())
        } else {
            Err("Compose engine not found".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_config(spec_json: *const StringHeader) -> *mut Promise {
    // Return resolved config as JSON string
    let spec_str = string_from_header(spec_json).unwrap_or_default();
    let spec: perry_container_compose::types::ComposeSpec = serde_json::from_str(&spec_str).unwrap();
    let promise = perry_runtime::js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let res = serde_json::to_string(&spec).unwrap();
        let s_ptr = perry_runtime::js_string_from_bytes(res.as_ptr(), res.len() as u32);
        Ok(perry_runtime::JSValue::string_ptr(s_ptr).bits())
    });
    promise
}
