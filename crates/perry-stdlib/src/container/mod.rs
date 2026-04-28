//! Container module for Perry

pub mod backend;
pub mod capability;
pub mod compose;
pub mod types;
pub mod verification;
pub mod workload;

pub use types::{
    ComposeHandle, ComposeSpec, ContainerError, ContainerHandle,
    ContainerInfo, ContainerLogs, ContainerSpec, ImageInfo, ListOrDict,
};

use perry_runtime::{js_promise_new, Promise, StringHeader};
pub use backend::{detect_backend, ContainerBackend};
use std::sync::{Arc, OnceLock};

static BACKEND: OnceLock<Arc<dyn ContainerBackend>> = OnceLock::new();

pub(crate) async fn get_global_backend() -> Result<&'static Arc<dyn ContainerBackend>, ContainerError> {
    if let Some(b) = BACKEND.get() { return Ok(b); }
    let b = detect_backend().await.map_err(ContainerError::from)?;
    let _ = BACKEND.set(b);
    Ok(BACKEND.get().unwrap())
}

unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() || (ptr as usize) < 0x1000 { return None; }
    let len = (*ptr).byte_len as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    Some(String::from_utf8_lossy(bytes).to_string())
}

unsafe fn string_to_js(s: &str) -> *const StringHeader {
    let bytes = s.as_bytes();
    perry_runtime::js_string_from_bytes(bytes.as_ptr(), bytes.len() as u32)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_run(spec_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec = match types::parse_container_spec(spec_ptr) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        let h = backend.run(&spec).await.map_err(|e| e.to_string())?;
        Ok(types::register_container_handle(h))
    });
    promise
}

#[no_mangle] pub unsafe extern "C" fn js_container_getBackend() -> *const StringHeader {
    if let Some(b) = BACKEND.get() { return string_to_js(b.backend_name()); }
    string_to_js("unknown")
}

#[no_mangle] pub unsafe extern "C" fn js_container_detectBackend() -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        match detect_backend().await {
            Ok(b) => Ok(serde_json::json!([{"name": b.backend_name(), "available": true, "reason": ""}]).to_string()),
            Err(e) => {
                let probed = if let perry_container_compose::error::ComposeError::NoBackendFound { probed } = e { probed } else { vec![] };
                Ok(serde_json::to_string(&probed).unwrap_or_else(|_| "[]".to_string()))
            }
        }
    }, |json| {
        let ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        perry_runtime::JSValue::string_ptr(ptr).bits()
    });
    promise
}

#[no_mangle] pub unsafe extern "C" fn js_container_build(spec_ptr: *const StringHeader, name_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = string_from_header(spec_ptr);
    let name = string_from_header(name_ptr).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let spec: types::ComposeServiceBuild = serde_json::from_str(&spec_json.unwrap_or_default()).map_err(|e| e.to_string())?;
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        backend.build(&spec, &name).await.map_err(|e| e.to_string())?;
        Ok(0u64)
    });
    promise
}

#[no_mangle] pub unsafe extern "C" fn js_container_composeUp(spec_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec = match types::parse_compose_spec(spec_ptr) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        compose::compose_up(spec, Arc::clone(backend)).await
    }, |handle| {
        let id = types::register_compose_handle(handle);
        perry_runtime::JSValue::number(id as f64).bits()
    });
    promise
}

#[no_mangle] pub unsafe extern "C" fn js_compose_down(id: i64, volumes: i32) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        compose::compose_down(id as u64, volumes != 0).await.map(|_| 0u64)
    });
    promise
}

#[no_mangle] pub unsafe extern "C" fn js_compose_ps(id: i64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let infos = compose::compose_ps(id as u64).await?;
        Ok(types::register_container_info_list(infos))
    });
    promise
}

#[no_mangle] pub unsafe extern "C" fn js_compose_logs(id: i64, svc_ptr: *const StringHeader, tail: i32) -> *mut Promise {
    let promise = js_promise_new();
    let svc = string_from_header(svc_ptr);
    let t = if tail >= 0 { Some(tail as u32) } else { None };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let logs = compose::compose_logs(id as u64, svc, t).await?;
        Ok(types::register_container_logs(logs))
    });
    promise
}

#[no_mangle] pub unsafe extern "C" fn js_compose_exec(id: i64, svc_ptr: *const StringHeader, cmd_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let svc = string_from_header(svc_ptr).unwrap_or_default();
    let cmd_json = string_from_header(cmd_ptr).unwrap_or_else(|| "[]".to_string());
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let cmd: Vec<String> = serde_json::from_str(&cmd_json).unwrap_or_default();
        let logs = compose::compose_exec(id as u64, svc, cmd).await?;
        Ok(types::register_container_logs(logs))
    });
    promise
}

#[no_mangle] pub unsafe extern "C" fn js_compose_start(id: i64, svcs_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let svcs_json = string_from_header(svcs_ptr).unwrap_or_else(|| "[]".to_string());
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let svcs: Vec<String> = serde_json::from_str(&svcs_json).unwrap_or_default();
        compose::compose_start(id as u64, svcs).await.map(|_| 0u64)
    });
    promise
}

#[no_mangle] pub unsafe extern "C" fn js_compose_stop(id: i64, svcs_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let svcs_json = string_from_header(svcs_ptr).unwrap_or_else(|| "[]".to_string());
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let svcs: Vec<String> = serde_json::from_str(&svcs_json).unwrap_or_default();
        compose::compose_stop(id as u64, svcs).await.map(|_| 0u64)
    });
    promise
}

#[no_mangle] pub unsafe extern "C" fn js_compose_restart(id: i64, svcs_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let svcs_json = string_from_header(svcs_ptr).unwrap_or_else(|| "[]".to_string());
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let svcs: Vec<String> = serde_json::from_str(&svcs_json).unwrap_or_default();
        compose::compose_restart(id as u64, svcs).await.map(|_| 0u64)
    });
    promise
}

#[no_mangle] pub unsafe extern "C" fn js_compose_config(id: i64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        compose::compose_config(id as u64).await
    }, |yaml| {
        let ptr = perry_runtime::js_string_from_bytes(yaml.as_ptr(), yaml.len() as u32);
        perry_runtime::JSValue::string_ptr(ptr).bits()
    });
    promise
}

#[no_mangle] pub extern "C" fn js_container_module_init() {}
