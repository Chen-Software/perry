//! Container module for Perry
//!
//! Provides OCI container management with platform-adaptive backend selection.

pub mod backend;
pub mod capability;
pub mod compose;
pub mod types;
pub mod verification;

use std::sync::Arc;
use tokio::sync::Mutex;
use once_cell::sync::Lazy;
use perry_runtime::{js_promise_new, Promise, StringHeader, JSValue};
use crate::common::spawn_for_promise;
pub use crate::container::backend::{detect_backend, ContainerBackend};
pub use crate::container::types::*;

static BACKEND: Lazy<Mutex<Option<Arc<dyn ContainerBackend>>>> = Lazy::new(|| Mutex::new(None));

pub async fn get_global_backend_instance() -> Result<Arc<dyn ContainerBackend>, ContainerError> {
    let mut lock = BACKEND.lock().await;
    if let Some(b) = &*lock { return Ok(Arc::clone(b)); }
    let b = detect_backend().await.map(Arc::from).map_err(ContainerError::from)?;
    *lock = Some(Arc::clone(&b));
    Ok(b)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_run(spec_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec = match parse_container_spec(spec_ptr) { Ok(s) => s, Err(e) => { spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) }); return promise; } };
    spawn_for_promise(promise as *mut u8, async move {
        let b = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let h = b.run(&spec).await.map_err(|e| e.to_string())?;
        Ok(register_container_handle(h))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_create(spec_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec = match parse_container_spec(spec_ptr) { Ok(s) => s, Err(e) => { spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) }); return promise; } };
    spawn_for_promise(promise as *mut u8, async move {
        let b = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let h = b.create(&spec).await.map_err(|e| e.to_string())?;
        Ok(register_container_handle(h))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_start(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();
    spawn_for_promise(promise as *mut u8, async move {
        let b = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        b.start(&id).await.map_err(|e| e.to_string())?;
        Ok(0u64)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_stop(id_ptr: *const StringHeader, timeout: i32) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();
    let t = if timeout < 0 { None } else { Some(timeout as u32) };
    spawn_for_promise(promise as *mut u8, async move {
        let b = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        b.stop(&id, t).await.map_err(|e| e.to_string())?;
        Ok(0u64)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_remove(id_ptr: *const StringHeader, force: i32) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();
    spawn_for_promise(promise as *mut u8, async move {
        let b = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        b.remove(&id, force != 0).await.map_err(|e| e.to_string())?;
        Ok(0u64)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_list(all: i32) -> *mut Promise {
    let promise = js_promise_new();
    spawn_for_promise(promise as *mut u8, async move {
        let b = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let list = b.list(all != 0).await.map_err(|e| e.to_string())?;
        Ok(register_container_info_list(list))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_inspect(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();
    spawn_for_promise(promise as *mut u8, async move {
        let b = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let info = b.inspect(&id).await.map_err(|e| e.to_string())?;
        Ok(register_container_info(info))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_logs(id_ptr: *const StringHeader, tail: i32, follow: i32) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();
    let t = if tail < 0 { None } else { Some(tail as u32) };
    spawn_for_promise(promise as *mut u8, async move {
        let b = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let logs = b.logs(&id, t, follow != 0).await.map_err(|e| e.to_string())?;
        Ok(register_container_logs(logs))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_exec(id_ptr: *const StringHeader, cmd_json: *const StringHeader, env_json: *const StringHeader, workdir: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();
    let cmd: Vec<String> = string_from_header(cmd_json).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    let env: Option<std::collections::HashMap<String, String>> = string_from_header(env_json).and_then(|s| serde_json::from_str(&s).ok());
    let wd = string_from_header(workdir);
    spawn_for_promise(promise as *mut u8, async move {
        let b = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let logs = b.exec(&id, &cmd, env.as_ref(), wd.as_deref(), None).await.map_err(|e| e.to_string())?;
        Ok(register_container_logs(logs))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_pullImage(reference_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let reference = string_from_header(reference_ptr).unwrap_or_default();
    spawn_for_promise(promise as *mut u8, async move {
        let b = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        b.pull_image(&reference).await.map_err(|e| e.to_string())?;
        Ok(0u64)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_listImages() -> *mut Promise {
    let promise = js_promise_new();
    spawn_for_promise(promise as *mut u8, async move {
        let b = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let list = b.list_images().await.map_err(|e| e.to_string())?;
        Ok(register_image_info_list(list))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_removeImage(reference_ptr: *const StringHeader, force: i32) -> *mut Promise {
    let promise = js_promise_new();
    let reference = string_from_header(reference_ptr).unwrap_or_default();
    spawn_for_promise(promise as *mut u8, async move {
        let b = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        b.remove_image(&reference, force != 0).await.map_err(|e| e.to_string())?;
        Ok(0u64)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_getBackend() -> *const StringHeader {
    let b = tokio::runtime::Handle::current().block_on(get_global_backend_instance());
    let name = b.map(|b| b.backend_name().to_string()).unwrap_or_else(|_| "unknown".to_string());
    let bytes = name.as_bytes();
    perry_runtime::js_string_from_bytes(bytes.as_ptr(), bytes.len() as u32)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_detectBackend() -> *mut Promise {
    let promise = js_promise_new();
    spawn_for_promise(promise as *mut u8, async move {
        let res = detect_backend().await;
        let json = match res {
            Ok(b) => serde_json::json!([{"name": b.backend_name(), "available": true, "reason": ""}]).to_string(),
            Err(perry_container_compose::error::ComposeError::NoBackendFound { probed }) => serde_json::to_string(&probed).unwrap_or_default(),
            Err(_) => format!("[]"),
        };
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        Ok(JSValue::string_ptr(str_ptr).bits())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_composeUp(spec_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec = match parse_compose_spec(spec_ptr) { Ok(s) => s, Err(e) => { spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) }); return promise; } };
    spawn_for_promise(promise as *mut u8, async move {
        let b = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let engine = perry_container_compose::compose::ComposeEngine::new(spec, "perry-stack".to_string(), b);
        let h = engine.up(&[], true, false, false).await.map_err(|e| compose_error_to_json(e.into()))?;
        Ok(register_compose_handle(h))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_up(spec_ptr: *const StringHeader) -> *mut Promise { js_container_composeUp(spec_ptr) }

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_down(handle_id: i64, volumes: i32) -> *mut Promise {
    let promise = js_promise_new();
    spawn_for_promise(promise as *mut u8, async move {
        let engine = perry_container_compose::compose::ComposeEngine::get_engine(handle_id as u64).ok_or("Invalid handle")?;
        engine.down(&[], false, volumes != 0).await.map_err(|e| compose_error_to_json(e.into()))?;
        Ok(0u64)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_ps(handle_id: i64) -> *mut Promise {
    let promise = js_promise_new();
    spawn_for_promise(promise as *mut u8, async move {
        let engine = perry_container_compose::compose::ComposeEngine::get_engine(handle_id as u64).ok_or("Invalid handle")?;
        let list = engine.ps().await.map_err(|e| e.to_string())?;
        Ok(register_container_info_list(list))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_logs(handle_id: i64, service_ptr: *const StringHeader, tail: i32) -> *mut Promise {
    let promise = js_promise_new();
    let service = string_from_header(service_ptr);
    let t = if tail < 0 { None } else { Some(tail as u32) };
    spawn_for_promise(promise as *mut u8, async move {
        let engine = perry_container_compose::compose::ComposeEngine::get_engine(handle_id as u64).ok_or("Invalid handle")?;
        let svcs = service.map(|s| vec![s]).unwrap_or_default();
        let logs_map = engine.logs(&svcs, t).await.map_err(|e| e.to_string())?;
        let mut stdout = String::new(); let mut stderr = String::new();
        for (name, logs) in logs_map { stdout.push_str(&format!("--- {} ---\n{}", name, logs.stdout)); stderr.push_str(&format!("--- {} ---\n{}", name, logs.stderr)); }
        Ok(register_container_logs(ContainerLogs { stdout, stderr }))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_exec(handle_id: i64, service_ptr: *const StringHeader, cmd_json: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let service = string_from_header(service_ptr).unwrap_or_default();
    let cmd: Vec<String> = string_from_header(cmd_json).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    spawn_for_promise(promise as *mut u8, async move {
        let engine = perry_container_compose::compose::ComposeEngine::get_engine(handle_id as u64).ok_or("Invalid handle")?;
        let logs = engine.exec(&service, &cmd).await.map_err(|e| e.to_string())?;
        Ok(register_container_logs(logs))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_config(spec_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec = match parse_compose_spec(spec_ptr) { Ok(s) => s, Err(e) => { spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) }); return promise; } };
    spawn_for_promise(promise as *mut u8, async move {
        let b = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let engine = perry_container_compose::compose::ComposeEngine::new(spec, "perry-stack".to_string(), b);
        let yaml = engine.config().map_err(|e| e.to_string())?;
        let str_ptr = perry_runtime::js_string_from_bytes(yaml.as_ptr(), yaml.len() as u32);
        Ok(JSValue::string_ptr(str_ptr).bits())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_start(handle_id: i64, svcs_json: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let svcs: Vec<String> = string_from_header(svcs_json).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    spawn_for_promise(promise as *mut u8, async move {
        let engine = perry_container_compose::compose::ComposeEngine::get_engine(handle_id as u64).ok_or("Invalid handle")?;
        engine.start(&svcs).await.map_err(|e| e.to_string())?; Ok(0u64)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_stop(handle_id: i64, svcs_json: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let svcs: Vec<String> = string_from_header(svcs_json).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    spawn_for_promise(promise as *mut u8, async move {
        let engine = perry_container_compose::compose::ComposeEngine::get_engine(handle_id as u64).ok_or("Invalid handle")?;
        engine.stop(&svcs).await.map_err(|e| e.to_string())?; Ok(0u64)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_restart(handle_id: i64, svcs_json: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let svcs: Vec<String> = string_from_header(svcs_json).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    spawn_for_promise(promise as *mut u8, async move {
        let engine = perry_container_compose::compose::ComposeEngine::get_engine(handle_id as u64).ok_or("Invalid handle")?;
        engine.restart(&svcs).await.map_err(|e| e.to_string())?; Ok(0u64)
    });
    promise
}

#[no_mangle] pub extern "C" fn js_container_module_init() {}
