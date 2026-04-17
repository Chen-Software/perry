//! Container module for Perry
//!
//! Provides OCI container management with platform-adaptive backend selection.

pub mod backend;
pub mod capability;
pub mod compose;
pub mod types;
pub mod verification;

// Re-export commonly used types
pub use types::{
    ComposeHandle, ComposeSpec, ContainerError, ContainerHandle,
    ContainerInfo, ContainerLogs, ContainerSpec, ImageInfo, ListOrDict,
};

use perry_runtime::{js_promise_new, Promise, StringHeader};
pub use backend::{detect_backend, ContainerBackend, get_global_backend};
use std::sync::Arc;
use std::collections::HashMap;

/// Helper to extract string from StringHeader pointer
unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() || (ptr as usize) < 0x1000 {
        return None;
    }
    let len = (*ptr).byte_len as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    Some(String::from_utf8_lossy(bytes).to_string())
}

/// Helper to create a JS string from a Rust string
unsafe fn string_to_js(s: &str) -> *const StringHeader {
    let bytes = s.as_bytes();
    perry_runtime::js_string_from_bytes(bytes.as_ptr(), bytes.len() as u32)
}

// ============ Container Lifecycle ============

/// Run a container from the given spec
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
        let handle = backend.run(&spec).await.map_err(|e| e.to_string())?;
        Ok(types::register_container_handle(handle) as u64)
    });
    promise
}

/// Create a container from the given spec without starting it
#[no_mangle]
pub unsafe extern "C" fn js_container_create(spec_ptr: *const StringHeader) -> *mut Promise {
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
        let handle = backend.create(&spec).await.map_err(|e| e.to_string())?;
        Ok(types::register_container_handle(handle) as u64)
    });
    promise
}

/// Start a previously created container
#[no_mangle]
pub unsafe extern "C" fn js_container_start(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) { Some(s) => s, None => { crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid ID".into()) }); return promise; } };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        backend.start(&id).await.map(|_| 0u64).map_err(|e| e.to_string())
    });
    promise
}

/// Stop a running container
#[no_mangle]
pub unsafe extern "C" fn js_container_stop(id_ptr: *const StringHeader, timeout: i32) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) { Some(s) => s, None => { crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid ID".into()) }); return promise; } };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        let t = if timeout < 0 { None } else { Some(timeout as u32) };
        backend.stop(&id, t).await.map(|_| 0u64).map_err(|e| e.to_string())
    });
    promise
}

/// Remove a container
#[no_mangle]
pub unsafe extern "C" fn js_container_remove(id_ptr: *const StringHeader, force: i32) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) { Some(s) => s, None => { crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid ID".into()) }); return promise; } };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        backend.remove(&id, force != 0).await.map(|_| 0u64).map_err(|e| e.to_string())
    });
    promise
}

/// List containers
#[no_mangle]
pub unsafe extern "C" fn js_container_list(all: i32) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        let list = backend.list(all != 0).await.map_err(|e| e.to_string())?;
        Ok(types::register_container_info_list(list))
    });
    promise
}

/// Inspect a container
#[no_mangle]
pub unsafe extern "C" fn js_container_inspect(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) { Some(s) => s, None => { crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid ID".into()) }); return promise; } };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        let info = backend.inspect(&id).await.map_err(|e| e.to_string())?;
        Ok(types::register_container_info(info))
    });
    promise
}

/// Get the current backend name
#[no_mangle]
pub unsafe extern "C" fn js_container_getBackend() -> *const StringHeader {
    string_to_js(backend::get_backend_name())
}

/// Detect backend and return probed info
#[no_mangle]
pub unsafe extern "C" fn js_container_detectBackend() -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        match detect_backend().await {
            Ok(b) => {
                let json = serde_json::json!([{ "name": b.backend_name(), "available": true, "reason": "" }]).to_string();
                Ok(json)
            }
            Err(probed) => Ok(serde_json::to_string(&probed).unwrap())
        }
    }, |json| {
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        perry_runtime::JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

// ============ Container Logs and Exec ============

/// Get logs from a container
#[no_mangle]
pub unsafe extern "C" fn js_container_logs(id_ptr: *const StringHeader, tail: i32) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) { Some(s) => s, None => { crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid ID".into()) }); return promise; } };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        let t = if tail < 0 { None } else { Some(tail as u32) };
        let logs = backend.logs(&id, t).await.map_err(|e| e.to_string())?;
        Ok(types::register_container_logs(logs))
    });
    promise
}

/// Execute a command in a container
#[no_mangle]
pub unsafe extern "C" fn js_container_exec(
    id_ptr: *const StringHeader,
    cmd_json_ptr: *const StringHeader,
    env_json_ptr: *const StringHeader,
    workdir_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) { Some(s) => s, None => { crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid ID".into()) }); return promise; } };
    let cmd_json = string_from_header(cmd_json_ptr);
    let env_json = string_from_header(env_json_ptr);
    let workdir = string_from_header(workdir_ptr);
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let cmd: Vec<String> = cmd_json.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let env: Option<HashMap<String, String>> = env_json.and_then(|s| serde_json::from_str(&s).ok());
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        let logs = backend.exec(&id, &cmd, env.as_ref(), workdir.as_deref()).await.map_err(|e| e.to_string())?;
        Ok(types::register_container_logs(logs))
    });
    promise
}

// ============ Image Management ============

/// Check if image exists
#[no_mangle]
pub unsafe extern "C" fn js_container_imageExists(reference_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let reference = match string_from_header(reference_ptr) { Some(s) => s, None => { crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid reference".into()) }); return promise; } };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        let exists = backend.image_exists(&reference).await.map_err(|e| e.to_string())?;
        Ok(if exists { 1u64 } else { 0u64 })
    });
    promise
}

/// Pull a container image
#[no_mangle]
pub unsafe extern "C" fn js_container_pullImage(reference_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let reference = match string_from_header(reference_ptr) { Some(s) => s, None => { crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid reference".into()) }); return promise; } };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        backend.pull_image(&reference).await.map(|_| 0u64).map_err(|e| e.to_string())
    });
    promise
}

/// List images
#[no_mangle]
pub unsafe extern "C" fn js_container_listImages() -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        let list = backend.list_images().await.map_err(|e| e.to_string())?;
        Ok(types::register_image_info_list(list))
    });
    promise
}

/// Remove an image
#[no_mangle]
pub unsafe extern "C" fn js_container_removeImage(reference_ptr: *const StringHeader, force: i32) -> *mut Promise {
    let promise = js_promise_new();
    let reference = match string_from_header(reference_ptr) { Some(s) => s, None => { crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid reference".into()) }); return promise; } };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        backend.remove_image(&reference, force != 0).await.map(|_| 0u64).map_err(|e| e.to_string())
    });
    promise
}

// ============ Compose Functions ============

/// Bring up a Compose stack
#[no_mangle]
pub unsafe extern "C" fn js_compose_up(spec_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec = match types::parse_compose_spec(spec_ptr) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        let wrapper = compose::ComposeWrapper::new(spec, backend);
        let handle = wrapper.up().await.map_err(|e| e.to_string())?;
        Ok(types::register_compose_handle(handle))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_down(handle_id: i64, volumes: i32) -> *mut Promise {
    let promise = js_promise_new();
    let handle = match types::take_compose_handle(handle_id as u64) { Some(h) => h, None => { crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid handle".into()) }); return promise; } };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        let wrapper = compose::ComposeWrapper::new(types::ComposeSpec::default(), backend);
        wrapper.down(&handle, volumes != 0).await.map(|_| 0u64).map_err(|e| e.to_string())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_ps(handle_id: i64) -> *mut Promise {
    let promise = js_promise_new();
    let handle = match types::get_compose_handle(handle_id as u64) { Some(h) => h, None => { crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid handle".into()) }); return promise; } };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        let wrapper = compose::ComposeWrapper::new(types::ComposeSpec::default(), backend);
        let list = wrapper.ps(&handle).await.map_err(|e| e.to_string())?;
        Ok(types::register_container_info_list(list))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_logs(handle_id: i64, service_ptr: *const StringHeader, tail: i32) -> *mut Promise {
    let promise = js_promise_new();
    let handle = match types::get_compose_handle(handle_id as u64) { Some(h) => h, None => { crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid handle".into()) }); return promise; } };
    let service = string_from_header(service_ptr);
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        let wrapper = compose::ComposeWrapper::new(types::ComposeSpec::default(), backend);
        let t = if tail < 0 { None } else { Some(tail as u32) };
        let logs = wrapper.logs(&handle, service.as_deref(), t).await.map_err(|e| e.to_string())?;
        Ok(types::register_container_logs(logs))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_config(spec_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec = match types::parse_compose_spec(spec_ptr) { Ok(s) => s, Err(e) => { crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) }); return promise; } };
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        let wrapper = compose::ComposeWrapper::new(spec, backend);
        wrapper.config().map_err(|e| e.to_string())
    }, |s| {
        let str_ptr = perry_runtime::js_string_from_bytes(s.as_ptr(), s.len() as u32);
        perry_runtime::JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_start(handle_id: i64, services_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let handle = match types::get_compose_handle(handle_id as u64) { Some(h) => h, None => { crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid handle".into()) }); return promise; } };
    let svcs_json = string_from_header(services_ptr);
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let svcs: Vec<String> = svcs_json.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        let wrapper = compose::ComposeWrapper::new(types::ComposeSpec::default(), backend);
        wrapper.start(&handle, &svcs).await.map(|_| 0u64).map_err(|e| e.to_string())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_stop(handle_id: i64, services_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let handle = match types::get_compose_handle(handle_id as u64) { Some(h) => h, None => { crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid handle".into()) }); return promise; } };
    let svcs_json = string_from_header(services_ptr);
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let svcs: Vec<String> = svcs_json.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        let wrapper = compose::ComposeWrapper::new(types::ComposeSpec::default(), backend);
        wrapper.stop(&handle, &svcs).await.map(|_| 0u64).map_err(|e| e.to_string())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_restart(handle_id: i64, services_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let handle = match types::get_compose_handle(handle_id as u64) { Some(h) => h, None => { crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid handle".into()) }); return promise; } };
    let svcs_json = string_from_header(services_ptr);
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let svcs: Vec<String> = svcs_json.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        let wrapper = compose::ComposeWrapper::new(types::ComposeSpec::default(), backend);
        wrapper.restart(&handle, &svcs).await.map(|_| 0u64).map_err(|e| e.to_string())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_exec(
    handle_id: i64,
    service_ptr: *const StringHeader,
    cmd_json_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let handle = match types::get_compose_handle(handle_id as u64) { Some(h) => h, None => { crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid handle".into()) }); return promise; } };
    let service = string_from_header(service_ptr);
    let cmd_json = string_from_header(cmd_json_ptr);
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let s = service.ok_or_else(|| "Invalid service".to_string())?;
        let cmd: Vec<String> = cmd_json.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        let wrapper = compose::ComposeWrapper::new(types::ComposeSpec::default(), backend);
        let logs = wrapper.exec(&handle, &s, &cmd).await.map_err(|e| e.to_string())?;
        Ok(types::register_container_logs(logs))
    });
    promise
}

// ============ Module Initialization ============

/// Initialize the container module (called during runtime startup)
#[no_mangle]
pub extern "C" fn js_container_module_init() {
}
