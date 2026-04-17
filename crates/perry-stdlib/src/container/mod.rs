//! Container module for Perry
//!
//! Provides OCI container management with platform-adaptive backend selection.

pub mod backend;
pub mod capability;
pub mod compose;
pub mod types;
pub mod verification;

// Internal helpers visible to other container modules
pub(crate) mod mod_priv {
    pub use super::backend::{ContainerBackend, get_backend_async};
    use std::sync::{Arc, OnceLock};
    use super::types::ContainerError;

    static BACKEND: OnceLock<Arc<dyn ContainerBackend>> = OnceLock::new();

    pub async fn get_global_backend_instance() -> Result<Arc<dyn ContainerBackend>, ContainerError> {
        if let Some(backend) = BACKEND.get() {
            return Ok(Arc::clone(backend));
        }

        let backend = get_backend_async().await?;
        let _ = BACKEND.set(Arc::clone(&backend));
        Ok(backend)
    }
}

use perry_runtime::{js_promise_new, Promise, StringHeader};
use self::mod_priv::get_global_backend_instance;

/// Helper to extract string from StringHeader pointer
unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() || (ptr as usize) < 0x1000 { return None; }
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

#[no_mangle]
pub unsafe extern "C" fn js_container_run(spec_json: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec = match types::parse_container_spec_json(spec_json) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        match backend.run(&spec).await {
            Ok(handle) => Ok(types::register_container_handle(handle)),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_create(spec_json: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec = match types::parse_container_spec_json(spec_json) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        match backend.create(&spec).await {
            Ok(handle) => Ok(types::register_container_handle(handle)),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_start(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        backend.start(&id).await.map(|_| 0u64).map_err(|e| e.to_string())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_stop(id_ptr: *const StringHeader, timeout: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();
    let t = if timeout < 0.0 { None } else { Some(timeout as u32) };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        backend.stop(&id, t).await.map(|_| 0u64).map_err(|e| e.to_string())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_remove(id_ptr: *const StringHeader, force: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        backend.remove(&id, force != 0.0).await.map(|_| 0u64).map_err(|e| e.to_string())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_list(all: f64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        match backend.list(all != 0.0).await {
            Ok(list) => Ok(types::register_container_info_list(list)),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_inspect(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        match backend.inspect(&id).await {
            Ok(info) => Ok(types::register_container_info(info)),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_logs(id_ptr: *const StringHeader, tail: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();
    let t = if tail < 0.0 { None } else { Some(tail as u32) };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        match backend.logs(&id, t).await {
            Ok(logs) => Ok(types::register_container_logs(logs)),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_exec(id_ptr: *const StringHeader, cmd_json: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();
    let cmd_str = string_from_header(cmd_json).unwrap_or_default();
    let cmd: Vec<String> = serde_json::from_str(&cmd_str).unwrap_or_else(|_| {
        cmd_str.split_whitespace().map(String::from).collect()
    });
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        match backend.exec(&id, &cmd, None, None).await {
            Ok(logs) => Ok(types::register_container_logs(logs)),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_pullImage(image_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let image = string_from_header(image_ptr).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        backend.pull_image(&image).await.map(|_| 0u64).map_err(|e| e.to_string())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_listImages() -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        match backend.list_images().await {
            Ok(list) => Ok(types::register_image_info_list(list)),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_removeImage(image_ptr: *const StringHeader, force: f64) -> *mut Promise {
    let promise = js_promise_new();
    let image = string_from_header(image_ptr).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        backend.remove_image(&image, force != 0.0).await.map(|_| 0u64).map_err(|e| e.to_string())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_getBackend() -> *const StringHeader {
    let rt = tokio::runtime::Handle::current();
    let name = rt.block_on(async {
        get_global_backend_instance().await
            .map(|b| b.backend_name().to_string())
            .unwrap_or_else(|_| "none".to_string())
    });
    string_to_js(&name)
}

// ============ Compose Functions ============

#[no_mangle]
pub unsafe extern "C" fn js_container_composeUp(spec_json: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec = match types::parse_compose_spec_json(spec_json) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let project_name = spec.name.clone().unwrap_or_else(|| "perry-stack".to_string());
        let engine = perry_container_compose::ComposeEngine::new(spec, project_name, backend);
        match engine.up(&[], true, true, false).await {
            Ok(_) => Ok(types::register_compose_engine(engine)),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_down(handle_id: u64, volumes: f64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_compose_engine(handle_id) {
            engine.down(&[], false, volumes != 0.0).await.map(|_| 0u64).map_err(|e| e.to_string())
        } else {
            Err("Invalid compose handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_ps(handle_id: u64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_compose_engine(handle_id) {
            match engine.ps().await {
                Ok(list) => Ok(types::register_container_info_list(list)),
                Err(e) => Err(e.to_string()),
            }
        } else {
            Err("Invalid compose handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_logs(
    handle_id: u64,
    service_ptr: *const StringHeader,
    tail: f64,
) -> *mut Promise {
    let promise = js_promise_new();
    let service = string_from_header(service_ptr);
    let services = service
        .as_ref()
        .map(|s| vec![s.clone()])
        .unwrap_or_default();
    let t = if tail < 0.0 { None } else { Some(tail as u32) };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_compose_engine(handle_id) {
            match engine.logs(&services, t).await {
                Ok(logs_map) => {
                    let mut stdout = String::new();
                    let mut stderr = String::new();
                    for logs in logs_map.values() {
                        stdout.push_str(&logs.stdout);
                        stdout.push('\n');
                        stderr.push_str(&logs.stderr);
                        stderr.push('\n');
                    }
                    Ok(types::register_container_logs(types::ContainerLogs {
                        stdout,
                        stderr,
                    }))
                }
                Err(e) => Err(e.to_string()),
            }
        } else {
            Err("Invalid compose handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_exec(
    handle_id: u64,
    service_ptr: *const StringHeader,
    cmd_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let service = string_from_header(service_ptr).unwrap_or_default();
    let cmd_str = string_from_header(cmd_ptr).unwrap_or_default();
    let cmd: Vec<String> = serde_json::from_str(&cmd_str).unwrap_or_else(|_| {
        cmd_str.split_whitespace().map(String::from).collect()
    });
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_compose_engine(handle_id) {
            match engine.exec(&service, &cmd, None, None).await {
                Ok(logs) => Ok(types::register_container_logs(logs)),
                Err(e) => Err(e.to_string()),
            }
        } else {
            Err("Invalid compose handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_config(handle_id: u64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_compose_engine(handle_id) {
            match serde_json::to_string(&engine.spec) {
                Ok(json) => Ok(types::register_string(json)),
                Err(e) => Err(e.to_string()),
            }
        } else {
            Err("Invalid compose handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_start(handle_id: u64, services_json: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let services_str = string_from_header(services_json).unwrap_or_default();
    let services: Vec<String> = serde_json::from_str(&services_str).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_compose_engine(handle_id) {
            engine.start(&services).await.map(|_| 0u64).map_err(|e| e.to_string())
        } else {
            Err("Invalid compose handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_stop(handle_id: u64, services_json: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let services_str = string_from_header(services_json).unwrap_or_default();
    let services: Vec<String> = serde_json::from_str(&services_str).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_compose_engine(handle_id) {
            engine.stop(&services).await.map(|_| 0u64).map_err(|e| e.to_string())
        } else {
            Err("Invalid compose handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_restart(handle_id: u64, services_json: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let services_str = string_from_header(services_json).unwrap_or_default();
    let services: Vec<String> = serde_json::from_str(&services_str).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_compose_engine(handle_id) {
            engine.restart(&services).await.map(|_| 0u64).map_err(|e| e.to_string())
        } else {
            Err("Invalid compose handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub extern "C" fn js_container_module_init() {
    // Force background initialization
    tokio::task::spawn(async {
        let _ = get_global_backend_instance().await;
    });
}
