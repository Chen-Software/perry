//! FFI functions for the container module.

pub mod backend;
pub mod capability;
pub mod compose;
pub mod types;
pub mod verification;

use perry_container_compose::backend::ContainerBackend;
use perry_runtime::{
    js_promise_new, Promise, StringHeader, js_string_from_bytes, JSValue,
};
use std::sync::{Arc, OnceLock};

static BACKEND: OnceLock<Result<Arc<dyn ContainerBackend + Send + Sync>, String>> = OnceLock::new();

pub(crate) unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let len = (*ptr).byte_len as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    Some(String::from_utf8_lossy(bytes).into_owned())
}

fn get_global_backend() -> Result<Arc<dyn ContainerBackend + Send + Sync>, String> {
    BACKEND
        .get_or_init(|| {
            tokio::runtime::Handle::current().block_on(async {
                match perry_container_compose::backend::detect_backend().await {
                    Ok(b) => Ok(Arc::from(b as Box<dyn ContainerBackend + Send + Sync>)),
                    Err(e) => Err(e.to_string()),
                }
            })
        })
        .clone()
}

// ============ FFI Functions ============

#[no_mangle]
pub unsafe extern "C" fn js_container_run(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async { Err("Missing spec JSON".to_string()) });
            return promise;
        }
    };
    let spec: types::ContainerSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            let err_msg = format!("Invalid spec JSON: {}", e);
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err(err_msg) });
            return promise;
        }
    };
    let backend = match get_global_backend() {
        Ok(b) => b,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err(e) });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let compose_spec = perry_container_compose::types::ContainerSpec {
            image: spec.image,
            name: spec.name,
            ports: spec.ports,
            volumes: spec.volumes,
            env: spec.env,
            cmd: spec.cmd,
            entrypoint: spec.entrypoint,
            network: spec.network,
            rm: spec.rm,
        };
        match backend.run(&compose_spec).await {
            Ok(h) => Ok(types::register_container_handle(types::ContainerHandle { id: h.id, name: h.name }) as u64),
            Err(e) => Err(e.to_string()),
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
            crate::common::spawn_for_promise(promise as *mut u8, async { Err("Missing spec JSON".to_string()) });
            return promise;
        }
    };
    let spec: types::ComposeSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            let err_msg = format!("Invalid spec JSON: {}", e);
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err(err_msg) });
            return promise;
        }
    };
    let backend = match get_global_backend() {
        Ok(b) => b,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err(e) });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let wrapper = compose::ComposeWrapper::new(spec.clone(), backend);
        match wrapper.up(&spec, &[]).await {
            Ok(h) => Ok(types::register_compose_handle(h) as u64),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_down(handle_id: u64, volumes: i32) -> *mut Promise {
    let promise = js_promise_new();
    let handle = match types::get_compose_handle(handle_id as i64) {
        Some(h) => h,
        None => {
            let err_msg = format!("Compose handle {} not found", handle_id);
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err(err_msg) });
            return promise;
        }
    };
    let backend = match get_global_backend() {
        Ok(b) => b,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err(e) });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let wrapper = compose::ComposeWrapper::new(types::ComposeSpec::default(), backend);
        match wrapper.down(handle, volumes != 0).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_ps(handle_id: u64) -> *mut Promise {
    let promise = js_promise_new();
    let handle = match types::get_compose_handle(handle_id as i64) {
        Some(h) => h,
        None => {
            let err_msg = format!("Compose handle {} not found", handle_id);
            crate::common::spawn_for_promise_deferred(promise as *mut u8, async move { Err::<Vec<types::ContainerInfo>, _>(err_msg) }, |_| 0);
            return promise;
        }
    };
    let backend = match get_global_backend() {
        Ok(b) => b,
        Err(e) => {
            crate::common::spawn_for_promise_deferred(promise as *mut u8, async move { Err::<Vec<types::ContainerInfo>, _>(e) }, |_| 0);
            return promise;
        }
    };
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let wrapper = compose::ComposeWrapper::new(types::ComposeSpec::default(), backend);
        wrapper.ps(handle).await.map_err(|e| e.to_string())
    }, |infos| {
        let json = serde_json::to_string(&infos).unwrap();
        let ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_logs(handle_id: u64, service_ptr: *const StringHeader, tail: i32) -> *mut Promise {
    let promise = js_promise_new();
    let handle = match types::get_compose_handle(handle_id as i64) {
        Some(h) => h,
        None => {
            let err_msg = format!("Compose handle {} not found", handle_id);
            crate::common::spawn_for_promise_deferred(promise as *mut u8, async move { Err::<std::collections::HashMap<String, String>, _>(err_msg) }, |_| 0);
            return promise;
        }
    };
    let service = string_from_header(service_ptr);
    let tail_opt = if tail >= 0 { Some(tail as u32) } else { None };
    let backend = match get_global_backend() {
        Ok(b) => b,
        Err(e) => {
            crate::common::spawn_for_promise_deferred(promise as *mut u8, async move { Err::<std::collections::HashMap<String, String>, _>(e) }, |_| 0);
            return promise;
        }
    };
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let wrapper = compose::ComposeWrapper::new(types::ComposeSpec::default(), backend);
        wrapper.logs(handle, service.as_deref(), tail_opt).await.map_err(|e| e.to_string())
    }, |logs| {
        let json = serde_json::to_string(&logs).unwrap();
        let ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_exec(handle_id: u64, service_ptr: *const StringHeader, cmd_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let handle = match types::get_compose_handle(handle_id as i64) {
        Some(h) => h,
        None => {
            let err_msg = format!("Compose handle {} not found", handle_id);
            crate::common::spawn_for_promise_deferred(promise as *mut u8, async move { Err::<types::ContainerLogs, _>(err_msg) }, |_| 0);
            return promise;
        }
    };
    let service = match string_from_header(service_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise_deferred(promise as *mut u8, async { Err::<types::ContainerLogs, _>("Missing service name".to_string()) }, |_| 0);
            return promise;
        }
    };
    let cmd_json = match string_from_header(cmd_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise_deferred(promise as *mut u8, async { Err::<types::ContainerLogs, _>("Missing command JSON".to_string()) }, |_| 0);
            return promise;
        }
    };
    let cmd: Vec<String> = match serde_json::from_str(&cmd_json) {
        Ok(c) => c,
        Err(e) => {
            let err_msg = format!("Invalid command JSON: {}", e);
            crate::common::spawn_for_promise_deferred(promise as *mut u8, async move { Err::<types::ContainerLogs, _>(err_msg) }, |_| 0);
            return promise;
        }
    };
    let backend = match get_global_backend() {
        Ok(b) => b,
        Err(e) => {
            crate::common::spawn_for_promise_deferred(promise as *mut u8, async move { Err::<types::ContainerLogs, _>(e) }, |_| 0);
            return promise;
        }
    };
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let wrapper = compose::ComposeWrapper::new(types::ComposeSpec::default(), backend);
        wrapper.exec(handle, &service, &cmd).await.map_err(|e| e.to_string())
    }, |logs| {
        let json = serde_json::to_string(&logs).unwrap();
        let ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_verifyImage(reference_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let reference = match string_from_header(reference_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise_deferred(promise as *mut u8, async { Err::<String, _>("Missing image reference".to_string()) }, |_| 0);
            return promise;
        }
    };
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        verification::verify_image(&reference).await.map_err(|e| e.to_string())
    }, |digest| {
        let ptr = js_string_from_bytes(digest.as_ptr(), digest.len() as u32);
        JSValue::string_ptr(ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_runCapability(command_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let command = match string_from_header(command_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise_deferred(promise as *mut u8, async { Err::<capability::CapabilityResult, _>("Missing command".to_string()) }, |_| 0);
            return promise;
        }
    };
    let backend = match get_global_backend() {
        Ok(b) => b,
        Err(e) => {
            crate::common::spawn_for_promise_deferred(promise as *mut u8, async move { Err::<capability::CapabilityResult, _>(e) }, |_| 0);
            return promise;
        }
    };
    let config = capability::CapabilityConfig::default();
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        match capability::run_capability(&backend, &command, &config).await {
            Ok(result) => Ok(result),
            Err(e) => Err(e.to_string()),
        }
    }, |result| {
        let logs = types::ContainerLogs {
            stdout: result.stdout,
            stderr: result.stderr,
            exit_code: result.exit_code,
        };
        let json = serde_json::to_string(&logs).unwrap();
        let ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_getBackend() -> *const StringHeader {
    let name = match get_global_backend() {
        Ok(backend) => backend.backend_name().to_string(),
        Err(_) => "none".to_string(),
    };
    perry_runtime::js_string_from_bytes(name.as_ptr(), name.len() as u32)
}

#[no_mangle]
pub extern "C" fn js_container_module_init() {
    let _ = get_global_backend();
}
