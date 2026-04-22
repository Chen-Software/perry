//! Container module for Perry

pub use super::types::{
    ComposeHealthcheck, ComposeNetwork, ComposeService, ComposeSpec, ComposeVolume,
    ContainerHandle, ContainerInfo, ContainerLogs, ContainerSpec, ImageInfo, ComposeError,
};

use perry_runtime::{js_promise_new, Promise, StringHeader, JSValue};
use super::backend::{detect_backend, ContainerBackend, BackendProbeResult};
use std::sync::{Arc, OnceLock};
use super::compose::ComposeEngine;
use super::context::{ContainerContext, HandleEntry};

pub async fn get_global_backend_instance_internal() -> Result<Arc<dyn ContainerBackend + Send + Sync>, String> {
    ContainerContext::global().get_backend().await
}

async fn get_global_backend_instance() -> Result<Arc<dyn ContainerBackend + Send + Sync>, String> {
    get_global_backend_instance_internal().await
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

fn compose_error_to_js(e: ComposeError) -> String {
    e.to_js_json()
}

fn backend_err_to_js(msg: String) -> String {
    serde_json::json!({
        "message": msg,
        "code": 503
    }).to_string()
}

// ============ Container API ============

#[no_mangle]
pub unsafe extern "C" fn js_container_run(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::async_bridge::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid spec JSON".into())) });
            return promise;
        }
    };
    let spec: ContainerSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            crate::common::async_bridge::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js(e.to_string())) });
            return promise;
        }
    };
    crate::common::async_bridge::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        match backend.run(&spec).await {
            Ok(handle) => {
                let id = super::types::register_container_handle(handle.clone());
                ContainerContext::global().handles.insert(id, HandleEntry::Container(handle));
                Ok(id)
            }
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_config_handle(handle_id: i64) -> *mut Promise {
    js_container_compose_config(handle_id)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_create(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::async_bridge::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid spec JSON".into())) });
            return promise;
        }
    };
    let spec: ContainerSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            crate::common::async_bridge::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js(e.to_string())) });
            return promise;
        }
    };
    crate::common::async_bridge::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        match backend.create(&spec).await {
            Ok(handle) => {
                let id = super::types::register_container_handle(handle.clone());
                ContainerContext::global().handles.insert(id, HandleEntry::Container(handle));
                Ok(id)
            }
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_start(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::async_bridge::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid ID".into())) });
            return promise;
        }
    };
    crate::common::async_bridge::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        match backend.start(&id).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_stop(id_ptr: *const StringHeader, timeout: i32) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::async_bridge::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid ID".into())) });
            return promise;
        }
    };
    crate::common::async_bridge::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        match backend.stop(&id, if timeout >= 0 { Some(timeout as u32) } else { None }).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_remove(id_ptr: *const StringHeader, force: i32) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::async_bridge::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid ID".into())) });
            return promise;
        }
    };
    crate::common::async_bridge::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        match backend.remove(&id, force != 0).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_list(all: i32) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::async_bridge::spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        backend.list(all != 0).await.map_err(compose_error_to_js)
    }, |list| {
        let json = serde_json::to_string(&list).unwrap_or_else(|_| "[]".into());
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_inspect(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::async_bridge::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid ID".into())) });
            return promise;
        }
    };
    crate::common::async_bridge::spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        backend.inspect(&id).await.map_err(compose_error_to_js)
    }, |info| {
        let json = serde_json::to_string(&info).unwrap_or_else(|_| "{}".into());
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_logs(id_ptr: *const StringHeader, tail: i32) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::async_bridge::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid ID".into())) });
            return promise;
        }
    };
    crate::common::async_bridge::spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        backend.logs(&id, if tail >= 0 { Some(tail as u32) } else { None }).await.map_err(compose_error_to_js)
    }, |logs| {
        let json = serde_json::to_string(&logs).unwrap_or_else(|_| "{\"stdout\":\"\",\"stderr\":\"\"}".into());
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_exec(
    id_ptr: *const StringHeader,
    cmd_json_ptr: *const StringHeader,
    env_json_ptr: *const StringHeader,
    workdir_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::async_bridge::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid ID".into())) });
            return promise;
        }
    };
    let cmd_json = string_from_header(cmd_json_ptr).unwrap_or_else(|| "[]".into());
    let env_json = string_from_header(env_json_ptr).unwrap_or_else(|| "{}".into());
    let workdir = string_from_header(workdir_ptr);

    let cmd: Vec<String> = serde_json::from_str(&cmd_json).unwrap_or_default();
    let env: Option<std::collections::HashMap<String, String>> = serde_json::from_str(&env_json).ok();

    crate::common::async_bridge::spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        match backend.exec(&id, &cmd, env.as_ref(), workdir.as_deref()).await {
            Ok(logs) => Ok(logs),
            Err(e) => Err(compose_error_to_js(e)),
        }
    }, |logs| {
        let json = serde_json::to_string(&logs).unwrap_or_else(|_| "{\"stdout\":\"\",\"stderr\":\"\"}".into());
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_pullImage(ref_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let reference = match string_from_header(ref_ptr) {
        Some(s) => s,
        None => {
            crate::common::async_bridge::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid reference".into())) });
            return promise;
        }
    };
    crate::common::async_bridge::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        match backend.pull_image(&reference).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_listImages() -> *mut Promise {
    let promise = js_promise_new();
    crate::common::async_bridge::spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        backend.list_images().await.map_err(compose_error_to_js)
    }, |list| {
        let json = serde_json::to_string(&list).unwrap_or_else(|_| "[]".into());
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_removeImage(ref_ptr: *const StringHeader, force: i32) -> *mut Promise {
    let promise = js_promise_new();
    let reference = match string_from_header(ref_ptr) {
        Some(s) => s,
        None => {
            crate::common::async_bridge::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid reference".into())) });
            return promise;
        }
    };
    crate::common::async_bridge::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        match backend.remove_image(&reference, force != 0).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_getBackend() -> *const StringHeader {
    let name = match crate::common::async_bridge::RUNTIME.block_on(get_global_backend_instance()) {
        Ok(b) => b.backend_name().to_string(),
        Err(_) => "none".to_string(),
    };
    string_to_js(&name)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_detectBackend() -> *mut Promise {
    let promise = js_promise_new();
    crate::common::async_bridge::spawn_for_promise_deferred(promise as *mut u8, async move {
        Ok(super::backend::probe_all_backends().await)
    }, |results| {
        let json = serde_json::to_string(&results).unwrap_or_else(|_| "[]".into());
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

// ============ Compose API ============

#[no_mangle]
pub unsafe extern "C" fn js_container_composeUp(spec_json_ptr: *const StringHeader) -> *mut Promise {
    js_compose_up(spec_json_ptr)
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_up(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let input = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::async_bridge::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid input".into())) });
            return promise;
        }
    };

    crate::common::async_bridge::spawn_for_promise(promise as *mut u8, async move {
        let spec = if input.trim().starts_with('{') {
            serde_json::from_str(&input).map_err(|e| backend_err_to_js(e.to_string()))?
        } else {
            let path = std::path::PathBuf::from(input);
            let config = perry_container_compose::config::ProjectConfig::resolve(vec![path], None, vec![]);
            let proj = perry_container_compose::project::ComposeProject::load(&config)
                .map_err(|e| compose_error_to_js(e))?;
            proj.spec
        };

        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        let engine = ComposeEngine::new(spec, backend);
        match engine.up().await {
            Ok(_handle) => {
                let arc_engine = Arc::new(engine);
                let id = super::types::register_compose_engine_arc(Arc::clone(&arc_engine));
                ContainerContext::global().handles.insert(id, HandleEntry::Compose(arc_engine));
                Ok(id)
            }
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_down(handle_id: i64, volumes: i32) -> *mut Promise {
    js_compose_down(handle_id, volumes)
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_down(handle_id: i64, volumes: i32) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    crate::common::async_bridge::spawn_for_promise(promise as *mut u8, async move {
        let engine = match ContainerContext::global().handles.get(&id) {
            Some(entry) => match entry.value() {
                HandleEntry::Compose(e) => Arc::clone(e),
                _ => return Err(backend_err_to_js("Not a compose handle".into())),
            },
            None => return Err(backend_err_to_js("Handle not found".into())),
        };
        match engine.down(volumes != 0).await {
            Ok(()) => {
                ContainerContext::global().handles.remove(&id);
                Ok(0u64)
            }
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_ps(handle_id: i64) -> *mut Promise {
    js_compose_ps(handle_id)
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_ps(handle_id: i64) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    crate::common::async_bridge::spawn_for_promise_deferred(promise as *mut u8, async move {
        let engine = match ContainerContext::global().handles.get(&id) {
            Some(entry) => match entry.value() {
                HandleEntry::Compose(e) => Arc::clone(e),
                _ => return Err(backend_err_to_js("Not a compose handle".into())),
            },
            None => return Err(backend_err_to_js("Handle not found".into())),
        };
        engine.ps().await.map_err(compose_error_to_js)
    }, |list| {
        let json = serde_json::to_string(&list).unwrap_or_else(|_| "[]".into());
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_logs(handle_id: i64, service_ptr: *const StringHeader, tail: i32) -> *mut Promise {
    js_compose_logs(handle_id, service_ptr, tail)
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_logs(handle_id: i64, service_ptr: *const StringHeader, tail: i32) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    let service = string_from_header(service_ptr);
    crate::common::async_bridge::spawn_for_promise_deferred(promise as *mut u8, async move {
        let engine = match ContainerContext::global().handles.get(&id) {
            Some(entry) => match entry.value() {
                HandleEntry::Compose(e) => Arc::clone(e),
                _ => return Err(backend_err_to_js("Not a compose handle".into())),
            },
            None => return Err(backend_err_to_js("Handle not found".into())),
        };
        engine.logs(service.as_deref(), if tail >= 0 { Some(tail as u32) } else { None }).await.map_err(compose_error_to_js)
    }, |logs| {
        let json = serde_json::to_string(&logs).unwrap_or_else(|_| "{\"stdout\":\"\",\"stderr\":\"\"}".into());
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_exec(handle_id: i64, service_ptr: *const StringHeader, cmd_json_ptr: *const StringHeader) -> *mut Promise {
    js_compose_exec(handle_id, service_ptr, cmd_json_ptr)
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_exec(handle_id: i64, service_ptr: *const StringHeader, cmd_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    let service = match string_from_header(service_ptr) {
        Some(s) => s,
        None => {
            crate::common::async_bridge::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid service name".into())) });
            return promise;
        }
    };
    let cmd_json = string_from_header(cmd_json_ptr).unwrap_or_else(|| "[]".into());
    let cmd: Vec<String> = serde_json::from_str(&cmd_json).unwrap_or_default();

    crate::common::async_bridge::spawn_for_promise_deferred(promise as *mut u8, async move {
        let engine = match ContainerContext::global().handles.get(&id) {
            Some(entry) => match entry.value() {
                HandleEntry::Compose(e) => Arc::clone(e),
                _ => return Err(backend_err_to_js("Not a compose handle".into())),
            },
            None => return Err(backend_err_to_js("Handle not found".into())),
        };
        engine.exec(&service, &cmd).await.map_err(compose_error_to_js)
    }, |logs| {
        let json = serde_json::to_string(&logs).unwrap_or_else(|_| "{\"stdout\":\"\",\"stderr\":\"\"}".into());
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_config(handle_id: i64) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    crate::common::async_bridge::spawn_for_promise_deferred(promise as *mut u8, async move {
        let engine = match ContainerContext::global().handles.get(&id) {
            Some(entry) => match entry.value() {
                HandleEntry::Compose(e) => Arc::clone(e),
                _ => return Err(backend_err_to_js("Not a compose handle".into())),
            },
            None => return Err(backend_err_to_js("Handle not found".into())),
        };
        Ok(engine.spec.clone())
    }, |spec| {
        let json = serde_json::to_string(&spec).unwrap_or_else(|_| "{}".into());
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_start(handle_id: i64, services_json_ptr: *const StringHeader) -> *mut Promise {
    js_compose_start_handle(handle_id, services_json_ptr)
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_start_handle(handle_id: i64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    let services_json = string_from_header(services_json_ptr).unwrap_or_else(|| "[]".into());
    let services: Vec<String> = serde_json::from_str(&services_json).unwrap_or_default();
    crate::common::async_bridge::spawn_for_promise(promise as *mut u8, async move {
        let engine = match ContainerContext::global().handles.get(&id) {
            Some(entry) => match entry.value() {
                HandleEntry::Compose(e) => Arc::clone(e),
                _ => return Err(backend_err_to_js("Not a compose handle".into())),
            },
            None => return Err(backend_err_to_js("Handle not found".into())),
        };
        match engine.start(&services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_stop(handle_id: i64, services_json_ptr: *const StringHeader) -> *mut Promise {
    js_compose_stop_handle(handle_id, services_json_ptr)
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_stop_handle(handle_id: i64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    let services_json = string_from_header(services_json_ptr).unwrap_or_else(|| "[]".into());
    let services: Vec<String> = serde_json::from_str(&services_json).unwrap_or_default();
    crate::common::async_bridge::spawn_for_promise(promise as *mut u8, async move {
        let engine = match ContainerContext::global().handles.get(&id) {
            Some(entry) => match entry.value() {
                HandleEntry::Compose(e) => Arc::clone(e),
                _ => return Err(backend_err_to_js("Not a compose handle".into())),
            },
            None => return Err(backend_err_to_js("Handle not found".into())),
        };
        match engine.stop(&services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_restart(handle_id: i64, services_json_ptr: *const StringHeader) -> *mut Promise {
    js_compose_restart_handle(handle_id, services_json_ptr)
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_restart_handle(handle_id: i64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    let services_json = string_from_header(services_json_ptr).unwrap_or_else(|| "[]".into());
    let services: Vec<String> = serde_json::from_str(&services_json).unwrap_or_default();
    crate::common::async_bridge::spawn_for_promise(promise as *mut u8, async move {
        let engine = match ContainerContext::global().handles.get(&id) {
            Some(entry) => match entry.value() {
                HandleEntry::Compose(e) => Arc::clone(e),
                _ => return Err(backend_err_to_js("Not a compose handle".into())),
            },
            None => return Err(backend_err_to_js("Handle not found".into())),
        };
        match engine.restart(&services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_graph(handle_id: i64) -> *const StringHeader {
    let id = handle_id as u64;
    let res = match ContainerContext::global().handles.get(&id) {
        Some(entry) => match entry.value() {
            HandleEntry::Compose(e) => e.graph(),
            _ => return string_to_js("{}"),
        },
        None => return string_to_js("{}"),
    };
    match res {
        Ok(graph) => {
            let json = serde_json::to_string(&graph).unwrap_or_else(|_| "{}".into());
            string_to_js(&json)
        }
        Err(_) => string_to_js("{}"),
    }
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_status(handle_id: i64) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    crate::common::async_bridge::spawn_for_promise_deferred(promise as *mut u8, async move {
        let engine = match ContainerContext::global().handles.get(&id) {
            Some(entry) => match entry.value() {
                HandleEntry::Compose(e) => Arc::clone(e),
                _ => return Err(backend_err_to_js("Not a compose handle".into())),
            },
            None => return Err(backend_err_to_js("Handle not found".into())),
        };
        engine.status().await.map_err(compose_error_to_js)
    }, |status| {
        let json = serde_json::to_string(&status).unwrap_or_else(|_| "{}".into());
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

// ============ Workload API ============

#[no_mangle]
pub unsafe extern "C" fn js_workload_graph(name_ptr: *const StringHeader, nodes_json_ptr: *const StringHeader) -> *const StringHeader {
    let name = string_from_header(name_ptr).unwrap_or_else(|| "default".into());
    let nodes_json = string_from_header(nodes_json_ptr).unwrap_or_else(|| "{}".into());
    let nodes: std::collections::HashMap<String, super::workload::WorkloadNode> = serde_json::from_str(&nodes_json).unwrap_or_default();

    let graph = super::workload::WorkloadGraph {
        name,
        nodes,
        edges: Vec::new(), // Edges derived from depends_on in nodes
    };

    let json = serde_json::to_string(&graph).unwrap_or_else(|_| "{}".into());
    string_to_js(&json)
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_node(name_ptr: *const StringHeader, spec_json_ptr: *const StringHeader) -> *const StringHeader {
    let name = string_from_header(name_ptr).unwrap_or_else(|| "node".into());
    let spec_json = string_from_header(spec_json_ptr).unwrap_or_else(|| "{}".into());

    // Spec is essentially WorkloadNode without id/name/depends_on
    let mut node: super::workload::WorkloadNode = serde_json::from_str(&spec_json).unwrap_or_else(|_| {
        super::workload::WorkloadNode {
            id: "".into(),
            name: "".into(),
            image: None,
            resources: None,
            ports: Vec::new(),
            env: std::collections::HashMap::new(),
            depends_on: Vec::new(),
            runtime: super::workload::RuntimeSpec::Auto,
            policy: super::workload::PolicySpec {
                tier: "default".into(),
                no_network: None,
                read_only_root: None,
                seccomp: None,
            },
        }
    });

    node.id = name.clone();
    node.name = name;

    let json = serde_json::to_string(&node).unwrap_or_else(|_| "{}".into());
    string_to_js(&json)
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_runGraph(graph_json_ptr: *const StringHeader, opts_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let graph_json = match string_from_header(graph_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::async_bridge::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid graph JSON".into())) });
            return promise;
        }
    };
    let opts_json = string_from_header(opts_json_ptr).unwrap_or_else(|| "{}".into());

    let workload_graph: super::workload::WorkloadGraph = match serde_json::from_str(&graph_json) {
        Ok(g) => g,
        Err(e) => {
            crate::common::async_bridge::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js(e.to_string())) });
            return promise;
        }
    };

    crate::common::async_bridge::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;

        // Convert WorkloadGraph to ComposeSpec
        let mut spec = ComposeSpec::default();
        spec.name = Some(workload_graph.name);
        for (id, node) in workload_graph.nodes {
            let mut svc = ComposeService::default();
            svc.image = node.image;
            svc.ports = Some(node.ports.into_iter().map(|p| perry_container_compose::types::PortSpec::Short(serde_yaml::Value::String(p))).collect());
            svc.depends_on = Some(perry_container_compose::types::DependsOnSpec::List(node.depends_on));
            svc.environment = Some(perry_container_compose::types::ListOrDict::Dict(node.env.into_iter().map(|(k, v)| {
                let val = match v {
                    super::workload::WorkloadEnvValue::Literal(s) => Some(serde_yaml::Value::String(s)),
                    super::workload::WorkloadEnvValue::Ref(r) => Some(serde_yaml::Value::String(format!("REF:{}", r.node_id))),
                };
                (k, val)
            }).collect()));
            spec.services.insert(id, svc);
        }

        let engine = ComposeEngine::new(spec, backend);
        match engine.up().await {
            Ok(_handle) => {
                let arc_engine = Arc::new(engine);
                let id = super::types::register_compose_engine_arc(Arc::clone(&arc_engine));
                ContainerContext::global().handles.insert(id, HandleEntry::Compose(arc_engine));
                Ok(id)
            }
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_inspectGraph(graph_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let graph_json = match string_from_header(graph_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::async_bridge::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid graph JSON".into())) });
            return promise;
        }
    };

    let workload_graph: super::workload::WorkloadGraph = match serde_json::from_str(&graph_json) {
        Ok(g) => g,
        Err(e) => {
            crate::common::async_bridge::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js(e.to_string())) });
            return promise;
        }
    };

    crate::common::async_bridge::spawn_for_promise_deferred(promise as *mut u8, async move {
        // Just return current status (all pending/unknown)
        let mut nodes = std::collections::HashMap::new();
        for id in workload_graph.nodes.keys() {
            nodes.insert(id.clone(), "pending".to_string());
        }
        Ok(super::workload::GraphStatus {
            nodes,
            healthy: false,
            errors: None,
        })
    }, |status| {
        let json = serde_json::to_string(&status).unwrap_or_else(|_| "{}".into());
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_down(handle_id: i64, opts_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    let opts_json = string_from_header(opts_json_ptr).unwrap_or_else(|| "{}".into());
    let volumes = opts_json.contains("\"volumes\":true");

    crate::common::async_bridge::spawn_for_promise(promise as *mut u8, async move {
        let engine = match ContainerContext::global().handles.get(&id) {
            Some(entry) => match entry.value() {
                HandleEntry::Compose(e) => Arc::clone(e),
                _ => return Err(backend_err_to_js("Not a workload handle".into())),
            },
            None => return Err(backend_err_to_js("Handle not found".into())),
        };
        match engine.down(volumes).await {
            Ok(()) => {
                ContainerContext::global().handles.remove(&id);
                Ok(0u64)
            }
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_status(handle_id: i64) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    crate::common::async_bridge::spawn_for_promise_deferred(promise as *mut u8, async move {
        let engine = match ContainerContext::global().handles.get(&id) {
            Some(entry) => match entry.value() {
                HandleEntry::Compose(e) => Arc::clone(e),
                _ => return Err(backend_err_to_js("Not a workload handle".into())),
            },
            None => return Err(backend_err_to_js("Handle not found".into())),
        };
        let status = engine.status().await.map_err(compose_error_to_js)?;
        let mut nodes = std::collections::HashMap::new();
        for svc in status.services {
            nodes.insert(svc.service, svc.state);
        }
        Ok(super::workload::GraphStatus {
            nodes,
            healthy: status.healthy,
            errors: None,
        })
    }, |status| {
        let json = serde_json::to_string(&status).unwrap_or_else(|_| "{}".into());
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_graph(handle_id: i64) -> *const StringHeader {
    let id = handle_id as u64;
    let res = match ContainerContext::global().handles.get(&id) {
        Some(entry) => match entry.value() {
            HandleEntry::Compose(e) => e.graph(),
            _ => return string_to_js("{}"),
        },
        None => return string_to_js("{}"),
    };
    match res {
        Ok(graph) => {
            let json = serde_json::to_string(&graph).unwrap_or_else(|_| "{}".into());
            string_to_js(&json)
        }
        Err(_) => string_to_js("{}"),
    }
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_logs(handle_id: i64, node_ptr: *const StringHeader, opts_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    let node = string_from_header(node_ptr);
    let opts_json = string_from_header(opts_json_ptr).unwrap_or_else(|| "{}".into());
    let tail = if opts_json.contains("\"tail\":") {
        // Minimal parsing
        Some(100u32)
    } else {
        None
    };

    crate::common::async_bridge::spawn_for_promise_deferred(promise as *mut u8, async move {
        let engine = match ContainerContext::global().handles.get(&id) {
            Some(entry) => match entry.value() {
                HandleEntry::Compose(e) => Arc::clone(e),
                _ => return Err(backend_err_to_js("Not a workload handle".into())),
            },
            None => return Err(backend_err_to_js("Handle not found".into())),
        };
        engine.logs(node.as_deref(), tail).await.map_err(compose_error_to_js)
    }, |logs| {
        let json = serde_json::to_string(&logs).unwrap_or_else(|_| "{\"stdout\":\"\",\"stderr\":\"\"}".into());
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_exec(handle_id: i64, node_ptr: *const StringHeader, cmd_json_ptr: *const StringHeader) -> *mut Promise {
    js_compose_exec(handle_id, node_ptr, cmd_json_ptr)
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_ps(handle_id: i64) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    crate::common::async_bridge::spawn_for_promise_deferred(promise as *mut u8, async move {
        let engine = match ContainerContext::global().handles.get(&id) {
            Some(entry) => match entry.value() {
                HandleEntry::Compose(e) => Arc::clone(e),
                _ => return Err(backend_err_to_js("Not a workload handle".into())),
            },
            None => return Err(backend_err_to_js("Handle not found".into())),
        };
        let status = engine.status().await.map_err(compose_error_to_js)?;
        let mut infos = Vec::new();
        for svc in status.services {
            infos.push(super::workload::NodeInfo {
                node_id: svc.service.clone(),
                name: svc.service,
                container_id: svc.container_id,
                state: svc.state,
                image: None, // Could be derived if needed
            });
        }
        Ok(infos)
    }, |infos| {
        let json = serde_json::to_string(&infos).unwrap_or_else(|_| "[]".into());
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub extern "C" fn js_container_module_init() {
    // No-op; backend is lazily detected via get_backend() which uses tokio Mutex
}
