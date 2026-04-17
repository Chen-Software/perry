//! `perry-stdlib` container bridge.

pub mod backend;
pub mod capability;
pub mod compose;
pub mod types;
pub mod verification;

use perry_runtime::{js_promise_new, Promise, StringHeader, js_string_from_bytes};
use std::sync::{Arc, OnceLock, Mutex};
use std::collections::HashMap;
use backend::{detect_backend, probe_all_backends, ContainerBackend};
use crate::common::spawn_for_promise;
use types::*;

static BACKEND: OnceLock<Arc<dyn ContainerBackend>> = OnceLock::new();
static BACKEND_INIT_MUTEX: Mutex<()> = Mutex::new(());

fn get_global_backend_instance() -> Result<Arc<dyn ContainerBackend>, String> {
    if let Some(backend) = BACKEND.get() {
        return Ok(Arc::clone(backend));
    }

    let _guard = BACKEND_INIT_MUTEX.lock().unwrap();
    if let Some(backend) = BACKEND.get() {
        return Ok(Arc::clone(backend));
    }

    let backend = crate::common::async_bridge::block_on(async {
        detect_backend().await
            .map(|b| Arc::new(b) as Arc<dyn ContainerBackend>)
            .map_err(|e| format!("No backend found: {:?}", e))
    })?;

    let _ = BACKEND.set(Arc::clone(&backend));
    Ok(backend)
}

unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() || (ptr as usize) < 0x1000 {
        return None;
    }
    let len = (*ptr).byte_len as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    Some(String::from_utf8_lossy(bytes).to_string())
}

// =============================================================================
// perry/container FFI
// =============================================================================

#[no_mangle]
pub unsafe extern "C" fn js_container_getBackend() -> *const StringHeader {
    let name = match get_global_backend_instance() {
        Ok(b) => b.backend_name().to_string(),
        Err(_) => "none".to_string(),
    };
    js_string_from_bytes(name.as_ptr(), name.len() as u32)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_run(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid JSON".to_string()) });
            return promise;
        }
    };

    let spec: ContainerSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e.to_string()) });
            return promise;
        }
    };

    let backend = match get_global_backend_instance() {
        Ok(b) => b,
        Err(e) => {
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };

    spawn_for_promise(promise as *mut u8, async move {
        match backend.run(&spec).await {
            Ok(handle) => Ok(register_container_handle(handle)),
            Err(e) => Err::<u64, String>(perry_container_compose::error::compose_error_to_js(e)),
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_create(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid JSON".to_string()) });
            return promise;
        }
    };

    let spec: ContainerSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e.to_string()) });
            return promise;
        }
    };

    let backend = match get_global_backend_instance() {
        Ok(b) => b,
        Err(e) => {
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };

    spawn_for_promise(promise as *mut u8, async move {
        match backend.create(&spec).await {
            Ok(handle) => Ok(register_container_handle(handle)),
            Err(e) => Err::<u64, String>(perry_container_compose::error::compose_error_to_js(e)),
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
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid ID".to_string()) });
            return promise;
        }
    };

    let backend = match get_global_backend_instance() {
        Ok(b) => b,
        Err(e) => {
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };

    spawn_for_promise(promise as *mut u8, async move {
        match backend.start(&id).await {
            Ok(_) => Ok(0u64),
            Err(e) => Err::<u64, String>(perry_container_compose::error::compose_error_to_js(e)),
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
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid ID".to_string()) });
            return promise;
        }
    };
    let timeout_opt = if timeout >= 0 { Some(timeout as u32) } else { None };

    let backend = match get_global_backend_instance() {
        Ok(b) => b,
        Err(e) => {
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };

    spawn_for_promise(promise as *mut u8, async move {
        match backend.stop(&id, timeout_opt).await {
            Ok(_) => Ok(0u64),
            Err(e) => Err::<u64, String>(perry_container_compose::error::compose_error_to_js(e)),
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
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid ID".to_string()) });
            return promise;
        }
    };
    let force_bool = force != 0;

    let backend = match get_global_backend_instance() {
        Ok(b) => b,
        Err(e) => {
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };

    spawn_for_promise(promise as *mut u8, async move {
        match backend.remove(&id, force_bool).await {
            Ok(_) => Ok(0u64),
            Err(e) => Err::<u64, String>(perry_container_compose::error::compose_error_to_js(e)),
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_list(all: i32) -> *mut Promise {
    let promise = js_promise_new();
    let all_bool = all != 0;

    let backend = match get_global_backend_instance() {
        Ok(b) => b,
        Err(e) => {
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };

    spawn_for_promise(promise as *mut u8, async move {
        match backend.list(all_bool).await {
            Ok(list) => Ok(register_container_info_list(list)),
            Err(e) => Err::<u64, String>(perry_container_compose::error::compose_error_to_js(e)),
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_inspect(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid ID".to_string()) });
            return promise;
        }
    };

    let backend = match get_global_backend_instance() {
        Ok(b) => b,
        Err(e) => {
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };

    spawn_for_promise(promise as *mut u8, async move {
        match backend.inspect(&id).await {
            Ok(info) => Ok(register_container_info(info)),
            Err(e) => Err::<u64, String>(perry_container_compose::error::compose_error_to_js(e)),
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_logs(id_ptr: *const StringHeader, tail: i32) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid ID".to_string()) });
            return promise;
        }
    };
    let tail_opt = if tail >= 0 { Some(tail as u32) } else { None };

    let backend = match get_global_backend_instance() {
        Ok(b) => b,
        Err(e) => {
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };

    spawn_for_promise(promise as *mut u8, async move {
        match backend.logs(&id, tail_opt).await {
            Ok(logs) => Ok(register_container_logs(logs)),
            Err(e) => Err::<u64, String>(perry_container_compose::error::compose_error_to_js(e)),
        }
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
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid ID".to_string()) });
            return promise;
        }
    };
    let cmd_json = match string_from_header(cmd_json_ptr) {
        Some(s) => s,
        None => "[]".to_string(),
    };
    let cmd: Vec<String> = serde_json::from_str(&cmd_json).unwrap_or_default();

    let env_json = string_from_header(env_json_ptr);
    let env: Option<HashMap<String, String>> = env_json.and_then(|s| serde_json::from_str(&s).ok());

    let workdir = string_from_header(workdir_ptr);

    let backend = match get_global_backend_instance() {
        Ok(b) => b,
        Err(e) => {
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };

    spawn_for_promise(promise as *mut u8, async move {
        match backend.exec(&id, &cmd, env.as_ref(), workdir.as_deref()).await {
            Ok(logs) => Ok(register_container_logs(logs)),
            Err(e) => Err::<u64, String>(perry_container_compose::error::compose_error_to_js(e)),
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_pullImage(ref_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let reference = match string_from_header(ref_ptr) {
        Some(s) => s,
        None => {
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid string".to_string()) });
            return promise;
        }
    };

    let backend = match get_global_backend_instance() {
        Ok(b) => b,
        Err(e) => {
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };

    spawn_for_promise(promise as *mut u8, async move {
        match backend.pull_image(&reference).await {
            Ok(_) => Ok(0u64),
            Err(e) => Err::<u64, String>(perry_container_compose::error::compose_error_to_js(e)),
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_listImages() -> *mut Promise {
    let promise = js_promise_new();
    let backend = match get_global_backend_instance() {
        Ok(b) => b,
        Err(e) => {
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };

    spawn_for_promise(promise as *mut u8, async move {
        match backend.list_images().await {
            Ok(list) => Ok(register_image_info_list(list)),
            Err(e) => Err::<u64, String>(perry_container_compose::error::compose_error_to_js(e)),
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_detectBackend() -> *mut Promise {
    let promise = js_promise_new();

    spawn_for_promise(promise as *mut u8, async move {
        let results = probe_all_backends().await;
        let json = serde_json::to_string(&results).unwrap_or_else(|_| "[]".to_string());
        let ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        Ok(box_string_ptr(ptr))
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_removeImage(ref_ptr: *const StringHeader, force: i32) -> *mut Promise {
    let promise = js_promise_new();
    let reference = match string_from_header(ref_ptr) {
        Some(s) => s,
        None => {
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid string".to_string()) });
            return promise;
        }
    };
    let force_bool = force != 0;

    let backend = match get_global_backend_instance() {
        Ok(b) => b,
        Err(e) => {
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };

    spawn_for_promise(promise as *mut u8, async move {
        match backend.remove_image(&reference, force_bool).await {
            Ok(_) => Ok(0u64),
            Err(e) => Err::<u64, String>(perry_container_compose::error::compose_error_to_js(e)),
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_composeUp(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid JSON".to_string()) });
            return promise;
        }
    };

    let spec: ComposeSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e.to_string()) });
            return promise;
        }
    };

    let backend = match get_global_backend_instance() {
        Ok(b) => b,
        Err(e) => {
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };

    spawn_for_promise(promise as *mut u8, async move {
        let mut engine = perry_container_compose::ComposeEngine::new(spec, "default".into(), backend);
        match engine.up(&[], true, false, false).await {
            Ok(_handle) => Ok(register_compose_handle(engine)),
            Err(e) => Err::<u64, String>(perry_container_compose::error::compose_error_to_js(e)),
        }
    });

    promise
}

// =============================================================================
// perry/compose FFI
// =============================================================================

#[no_mangle]
pub unsafe extern "C" fn js_compose_up(spec_json_ptr: *const StringHeader) -> *mut Promise {
    js_container_composeUp(spec_json_ptr)
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_down(handle_bits: f64, volumes: i32) -> *mut Promise {
    let promise = js_promise_new();
    let volumes_bool = volumes != 0;
    let handle_id = unbox_id(handle_bits);

    let engine = match take_compose_handle(handle_id) {
        Some(e) => e,
        None => {
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid handle".to_string()) });
            return promise;
        }
    };

    spawn_for_promise(promise as *mut u8, async move {
        match engine.down(volumes_bool, false).await {
            Ok(_) => Ok(0u64),
            Err(e) => Err::<u64, String>(perry_container_compose::error::compose_error_to_js(e)),
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_ps(handle_bits: f64) -> *mut Promise {
    let promise = js_promise_new();
    let handle_id = unbox_id(handle_bits);
    let engine_ref = match get_compose_handle(handle_id) {
        Some(e) => e,
        None => {
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid handle".to_string()) });
            return promise;
        }
    };

    let backend = Arc::clone(&engine_ref.backend);
    let containers: Vec<(String, String)> = engine_ref.containers.iter().map(|r| (r.key().clone(), r.value().clone())).collect();
    drop(engine_ref);

    spawn_for_promise(promise as *mut u8, async move {
        let mut results = Vec::new();
        for (_svc, id) in containers {
            if let Ok(info) = backend.inspect(&id).await {
                results.push(info);
            }
        }
        Ok(register_container_info_list(results))
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_logs(handle_bits: f64, service_ptr: *const StringHeader, tail: i32) -> *mut Promise {
    let promise = js_promise_new();
    let handle_id = unbox_id(handle_bits);
    let service = string_from_header(service_ptr);
    let tail_opt = if tail >= 0 { Some(tail as u32) } else { None };

    let engine_ref = match get_compose_handle(handle_id) {
        Some(e) => e,
        None => {
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid handle".to_string()) });
            return promise;
        }
    };

    let backend = Arc::clone(&engine_ref.backend);
    let container_id = service.as_ref().and_then(|s| engine_ref.containers.get(s).map(|r| r.value().clone()));
    drop(engine_ref);

    spawn_for_promise(promise as *mut u8, async move {
        if let Some(id) = container_id {
            match backend.logs(&id, tail_opt).await {
                Ok(logs) => Ok(register_container_logs(logs)),
                Err(e) => Err::<u64, String>(perry_container_compose::error::compose_error_to_js(e)),
            }
        } else {
            Err::<u64, String>("Service not found or no service specified".to_string())
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_exec(
    handle_bits: f64,
    service_ptr: *const StringHeader,
    cmd_json_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let handle_id = unbox_id(handle_bits);
    let service = match string_from_header(service_ptr) {
        Some(s) => s,
        None => {
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid service name".to_string()) });
            return promise;
        }
    };
    let cmd_json = match string_from_header(cmd_json_ptr) {
        Some(s) => s,
        None => "[]".to_string(),
    };
    let cmd: Vec<String> = serde_json::from_str(&cmd_json).unwrap_or_default();

    let engine_ref = match get_compose_handle(handle_id) {
        Some(e) => e,
        None => {
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid handle".to_string()) });
            return promise;
        }
    };

    let backend = Arc::clone(&engine_ref.backend);
    let container_id = engine_ref.containers.get(&service).map(|r| r.value().clone());
    drop(engine_ref);

    spawn_for_promise(promise as *mut u8, async move {
        if let Some(id) = container_id {
            match backend.exec(&id, &cmd, None, None).await {
                Ok(logs) => Ok(register_container_logs(logs)),
                Err(e) => Err::<u64, String>(perry_container_compose::error::compose_error_to_js(e)),
            }
        } else {
            Err::<u64, String>(format!("Service '{}' not found", service))
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_config(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid JSON".to_string()) });
            return promise;
        }
    };

    // For config, we just return the spec back as a JSON string for now, validated.
    let spec: ComposeSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e.to_string()) });
            return promise;
        }
    };

    spawn_for_promise(promise as *mut u8, async move {
        let validated_json = serde_json::to_string(&spec).unwrap_or_default();
        let ptr = js_string_from_bytes(validated_json.as_ptr(), validated_json.len() as u32);
        Ok(box_string_ptr(ptr))
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_start(handle_bits: f64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let handle_id = unbox_id(handle_bits);
    let services_json = match string_from_header(services_json_ptr) {
        Some(s) => s,
        None => "[]".to_string(),
    };
    let services: Vec<String> = serde_json::from_str(&services_json).unwrap_or_default();

    let engine_ref = match get_compose_handle(handle_id) {
        Some(e) => e,
        None => {
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid handle".to_string()) });
            return promise;
        }
    };

    // In a real implementation we would call engine.start(&services)
    // For now we delegate to the backend for each service
    let backend = Arc::clone(&engine_ref.backend);
    let container_ids: Vec<String> = if services.is_empty() {
        engine_ref.containers.iter().map(|r| r.value().clone()).collect()
    } else {
        services.iter().filter_map(|s| engine_ref.containers.get(s).map(|r| r.value().clone())).collect()
    };
    drop(engine_ref);

    spawn_for_promise(promise as *mut u8, async move {
        for id in container_ids {
            backend.start(&id).await.ok();
        }
        Ok(0u64)
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_stop(handle_bits: f64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let handle_id = unbox_id(handle_bits);
    let services_json = match string_from_header(services_json_ptr) {
        Some(s) => s,
        None => "[]".to_string(),
    };
    let services: Vec<String> = serde_json::from_str(&services_json).unwrap_or_default();

    let engine_ref = match get_compose_handle(handle_id) {
        Some(e) => e,
        None => {
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid handle".to_string()) });
            return promise;
        }
    };

    let backend = Arc::clone(&engine_ref.backend);
    let container_ids: Vec<String> = if services.is_empty() {
        engine_ref.containers.iter().map(|r| r.value().clone()).collect()
    } else {
        services.iter().filter_map(|s| engine_ref.containers.get(s).map(|r| r.value().clone())).collect()
    };
    drop(engine_ref);

    spawn_for_promise(promise as *mut u8, async move {
        for id in container_ids {
            backend.stop(&id, None).await.ok();
        }
        Ok(0u64)
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_restart(handle_bits: f64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let handle_id = unbox_id(handle_bits);
    let services_json = match string_from_header(services_json_ptr) {
        Some(s) => s,
        None => "[]".to_string(),
    };
    let services: Vec<String> = serde_json::from_str(&services_json).unwrap_or_default();

    let engine_ref = match get_compose_handle(handle_id) {
        Some(e) => e,
        None => {
            spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid handle".to_string()) });
            return promise;
        }
    };

    let backend = Arc::clone(&engine_ref.backend);
    let container_ids: Vec<String> = if services.is_empty() {
        engine_ref.containers.iter().map(|r| r.value().clone()).collect()
    } else {
        services.iter().filter_map(|s| engine_ref.containers.get(s).map(|r| r.value().clone())).collect()
    };
    drop(engine_ref);

    spawn_for_promise(promise as *mut u8, async move {
        for id in container_ids {
            backend.stop(&id, None).await.ok();
            backend.start(&id).await.ok();
        }
        Ok(0u64)
    });

    promise
}
