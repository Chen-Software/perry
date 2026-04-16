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
pub use backend::{detect_backend, ContainerBackend};
use std::sync::OnceLock;
use std::sync::Arc;
use std::collections::HashMap;

// Global backend instance - initialized once at first use
static BACKEND: OnceLock<Arc<dyn ContainerBackend>> = OnceLock::new();

/// Get or initialize the global backend instance
async fn get_global_backend() -> Result<&'static Arc<dyn ContainerBackend>, ContainerError> {
    if let Some(b) = BACKEND.get() {
        return Ok(b);
    }

    let b = detect_backend().await
        .map(|b| Arc::new(b) as Arc<dyn ContainerBackend>)
        .map_err(|probed| ContainerError::NoBackendFound { probed })?;

    let _ = BACKEND.set(b);
    Ok(BACKEND.get().unwrap())
}

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
/// FFI: js_container_run(spec_val: f64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_run(spec_val: f64) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json_ptr = perry_runtime::js_json_stringify(spec_val, 1);

    let spec = match types::parse_container_spec(spec_json_ptr) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(e)
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.run(&spec).await {
            Ok(handle) => {
                let handle_id = types::register_container_handle(handle);
                Ok(handle_id as u64)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Create a container from the given spec without starting it
/// FFI: js_container_create(spec_val: f64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_create(spec_val: f64) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json_ptr = perry_runtime::js_json_stringify(spec_val, 1);

    let spec = match types::parse_container_spec(spec_json_ptr) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(e)
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.create(&spec).await {
            Ok(handle) => {
                let handle_id = types::register_container_handle(handle);
                Ok(handle_id as u64)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Start a previously created container
/// FFI: js_container_start(id: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_start(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid container ID".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.start(&id).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Stop a running container
/// FFI: js_container_stop(id: *const StringHeader, timeout: f64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_stop(id_ptr: *const StringHeader, timeout: f64) -> *mut Promise {
    let promise = js_promise_new();

    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid container ID".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let timeout_opt = if timeout < 0.0 { None } else { Some(timeout as u32) };
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.stop(&id, timeout_opt).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Remove a container
/// FFI: js_container_remove(id: *const StringHeader, force: f64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_remove(id_ptr: *const StringHeader, force: f64) -> *mut Promise {
    let promise = js_promise_new();

    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid container ID".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.remove(&id, force != 0.0).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// List containers
/// FFI: js_container_list(all: f64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_list(all: f64) -> *mut Promise {
    let promise = js_promise_new();

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err(e.to_string()),
        };
        match backend.list(all != 0.0).await {
            Ok(containers) => Ok(serde_json::to_string(&containers).unwrap()),
            Err(e) => Err(e.to_string()),
        }
    }, |json| {
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        let jsv = perry_runtime::js_json_parse(str_ptr);
        jsv.bits()
    });

    promise
}

/// Inspect a container
/// FFI: js_container_inspect(id: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_inspect(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid container ID".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err(e.to_string()),
        };
        match backend.inspect(&id).await {
            Ok(info) => Ok(serde_json::to_string(&info).unwrap()),
            Err(e) => Err(e.to_string()),
        }
    }, |json| {
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        let jsv = perry_runtime::js_json_parse(str_ptr);
        jsv.bits()
    });

    promise
}

/// Get the current backend name
/// FFI: js_container_getBackend() -> *const StringHeader
#[no_mangle]
pub unsafe extern "C" fn js_container_getBackend() -> *const StringHeader {
    // Note: this is synchronous and might return "unknown" if not initialized
    if let Some(b) = BACKEND.get() {
        return string_to_js(b.backend_name());
    }
    string_to_js("unknown")
}

/// Detect backend and return probed info
/// FFI: js_container_detectBackend() -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_detectBackend() -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        match detect_backend().await {
            Ok(b) => {
                let name = b.backend_name().to_string();
                let json = serde_json::json!([{
                    "name": name,
                    "available": true,
                    "reason": ""
                }]).to_string();
                Ok(json)
            }
            Err(probed) => {
                let json = serde_json::to_string(&probed).unwrap_or_default();
                Ok(json) // Resolve with probe info array on failure to find any
            }
        }
    }, |json| {
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        perry_runtime::JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

// ============ Container Logs and Exec ============

/// Get logs from a container
/// FFI: js_container_logs(id: *const StringHeader, tail: f64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_logs(id_ptr: *const StringHeader, tail: f64) -> *mut Promise {
    let promise = js_promise_new();

    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid container ID".to_string())
            });
            return promise;
        }
    };

    let tail_opt = if tail >= 0.0 { Some(tail as u32) } else { None };

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err(e.to_string()),
        };
        match backend.logs(&id, tail_opt).await {
            Ok(logs) => Ok(serde_json::to_string(&logs).unwrap()),
            Err(e) => Err(e.to_string()),
        }
    }, |json| {
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        let jsv = perry_runtime::js_json_parse(str_ptr);
        jsv.bits()
    });

    promise
}

/// Execute a command in a container
/// FFI: js_container_exec(id: *const StringHeader, cmd_json: *const StringHeader, env_json: *const StringHeader, workdir: *const StringHeader) -> *mut Promise
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
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid container ID".to_string())
            });
            return promise;
        }
    };

    let cmd_json = string_from_header(cmd_json_ptr);
    let env_json = string_from_header(env_json_ptr);
    let workdir = string_from_header(workdir_ptr);

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let cmd: Vec<String> = cmd_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let env: Option<HashMap<String, String>> = env_json
            .and_then(|s| serde_json::from_str(&s).ok());

        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err(e.to_string()),
        };
        match backend.exec(&id, &cmd, env.as_ref(), workdir.as_deref()).await {
            Ok(logs) => Ok(serde_json::to_string(&logs).unwrap()),
            Err(e) => Err(e.to_string()),
        }
    }, |json| {
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        let jsv = perry_runtime::js_json_parse(str_ptr);
        jsv.bits()
    });

    promise
}

// ============ Image Management ============

/// Pull a container image
/// FFI: js_container_pullImage(reference: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_pullImage(reference_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let reference = match string_from_header(reference_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid image reference".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.pull_image(&reference).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// List images
/// FFI: js_container_listImages() -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_listImages() -> *mut Promise {
    let promise = js_promise_new();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.list_images().await {
            Ok(images) => {
                let handle_id = types::register_image_info_list(images);
                Ok(handle_id as u64)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Remove an image
/// FFI: js_container_removeImage(reference: *const StringHeader, force: f64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_removeImage(reference_ptr: *const StringHeader, force: f64) -> *mut Promise {
    let promise = js_promise_new();

    let reference = match string_from_header(reference_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid image reference".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.remove_image(&reference, force != 0.0).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

// ============ Compose Functions ============

/// Helper to extract a string array from a NaN-boxed JSValue
unsafe fn jsvalue_to_string_array(val: f64) -> Vec<String> {
    let jsv = perry_runtime::JSValue::from_bits(val.to_bits());
    if jsv.is_pointer() {
        let arr_ptr = jsv.as_pointer::<perry_runtime::ArrayHeader>();
        if (arr_ptr as usize) < 0x10000 { return Vec::new(); }
        let len = perry_runtime::js_array_length(arr_ptr);
        let mut result = Vec::with_capacity(len as usize);
        for i in 0..len {
            let item = perry_runtime::js_array_get_f64(arr_ptr, i);
            let item_jsv = perry_runtime::JSValue::from_bits(item.to_bits());
            if item_jsv.is_string() {
                if let Some(s) = string_from_header(item_jsv.as_string_ptr()) {
                    result.push(s);
                }
            }
        }
        result
    } else {
        Vec::new()
    }
}

/// Bring up a Compose stack
/// FFI: js_container_composeUp(spec_val: f64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_composeUp(spec_val: f64) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json_ptr = perry_runtime::js_json_stringify(spec_val, 1);

    let spec = match types::parse_compose_spec(spec_json_ptr) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(e)
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match compose::compose_up(spec, backend).await {
            Ok(engine) => {
                let handle_id = types::register_compose_engine(engine);
                Ok(handle_id as u64)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Bring up a Compose stack (alias for up)
/// FFI: js_container_compose_up(spec_val: f64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_up(spec_val: f64) -> *mut Promise {
    js_container_composeUp(spec_val)
}

/// Helper to extract a boolean field from a JS object
unsafe fn get_object_field_bool(obj_ptr: *const perry_runtime::ObjectHeader, key: &str) -> bool {
    let key_ptr = perry_runtime::js_string_from_bytes(key.as_ptr(), key.len() as u32);
    let val = perry_runtime::js_object_get_field_by_name_f64(obj_ptr, key_ptr);
    perry_runtime::js_is_truthy(val) != 0
}

/// Helper to extract a string field from a JS object
unsafe fn get_object_field_string(obj_ptr: *const perry_runtime::ObjectHeader, key: &str) -> Option<String> {
    let key_ptr = perry_runtime::js_string_from_bytes(key.as_ptr(), key.len() as u32);
    let val = perry_runtime::js_object_get_field_by_name_f64(obj_ptr, key_ptr);
    let jsv = perry_runtime::JSValue::from_bits(val.to_bits());
    if jsv.is_string() {
        string_from_header(jsv.as_string_ptr())
    } else {
        None
    }
}

/// Helper to extract a u32 field from a JS object
unsafe fn get_object_field_u32(obj_ptr: *const perry_runtime::ObjectHeader, key: &str) -> Option<u32> {
    let key_ptr = perry_runtime::js_string_from_bytes(key.as_ptr(), key.len() as u32);
    let val = perry_runtime::js_object_get_field_by_name_f64(obj_ptr, key_ptr);
    let jsv = perry_runtime::JSValue::from_bits(val.to_bits());
    if jsv.is_number() {
        Some(jsv.as_number() as u32)
    } else {
        None
    }
}

/// Helper to extract a string array from a JS object field
unsafe fn get_object_field_string_array(obj_ptr: *const perry_runtime::ObjectHeader, key: &str) -> Vec<String> {
    let key_ptr = perry_runtime::js_string_from_bytes(key.as_ptr(), key.len() as u32);
    let val = perry_runtime::js_object_get_field_by_name_f64(obj_ptr, key_ptr);
    let jsv = perry_runtime::JSValue::from_bits(val.to_bits());
    if jsv.is_pointer() {
        let arr_ptr = jsv.as_pointer::<perry_runtime::ArrayHeader>();
        let len = perry_runtime::js_array_length(arr_ptr);
        let mut result = Vec::new();
        for i in 0..len {
            let item = perry_runtime::js_array_get_f64(arr_ptr, i);
            let item_jsv = perry_runtime::JSValue::from_bits(item.to_bits());
            if item_jsv.is_string() {
                if let Some(s) = string_from_header(item_jsv.as_string_ptr()) {
                    result.push(s);
                }
            }
        }
        result
    } else {
        Vec::new()
    }
}

/// Stop and remove compose stack.
/// FFI: js_container_compose_down(handle_id: i64, options: f64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_down(handle_id: i64, options: f64) -> *mut Promise {
    let promise = js_promise_new();

    let engine = match types::take_compose_engine(handle_id as u64) {
        Some(h) => h,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    let opt_jsv = perry_runtime::JSValue::from_bits(options.to_bits());
    let (volumes, services) = if opt_jsv.is_pointer() {
        let opt_ptr = opt_jsv.as_pointer::<perry_runtime::ObjectHeader>();
        let v = get_object_field_bool(opt_ptr, "volumes");
        let s = get_object_field_string_array(opt_ptr, "services");
        (v, s)
    } else {
        (false, Vec::new())
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match engine.down(&services, false, volumes).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Get container info for compose stack
/// FFI: js_container_compose_ps(handle_id: i64, options: f64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_ps(handle_id: i64, _options: f64) -> *mut Promise {
    let promise = js_promise_new();

    let engine = match types::get_compose_engine(handle_id as u64) {
        Some(h) => Arc::clone(h),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        match engine.ps().await {
            Ok(containers) => Ok(serde_json::to_string(&containers).unwrap()),
            Err(e) => Err(e.to_string()),
        }
    }, |json| {
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        let jsv = perry_runtime::js_json_parse(str_ptr);
        jsv.bits()
    });

    promise
}

/// Get logs from compose stack
/// FFI: js_container_compose_logs(handle_id: i64, options: f64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_logs(
    handle_id: i64,
    options: f64,
) -> *mut Promise {
    let promise = js_promise_new();

    let engine = match types::get_compose_engine(handle_id as u64) {
        Some(h) => Arc::clone(h),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    let opt_jsv = perry_runtime::JSValue::from_bits(options.to_bits());
    let (service, tail) = if opt_jsv.is_pointer() {
        let opt_ptr = opt_jsv.as_pointer::<perry_runtime::ObjectHeader>();
        let s = get_object_field_string(opt_ptr, "service");
        let t = get_object_field_u32(opt_ptr, "tail");
        (s, t)
    } else {
        (None, None)
    };

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        match engine.logs(service.as_deref(), tail).await {
            Ok(logs) => Ok(serde_json::to_string(&logs).unwrap()),
            Err(e) => Err(e.to_string()),
        }
    }, |json| {
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        let jsv = perry_runtime::js_json_parse(str_ptr);
        jsv.bits()
    });

    promise
}

/// Execute command in compose service
/// FFI: js_container_compose_exec(handle_id: i64, service: *const StringHeader, cmd_val: f64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_exec(
    handle_id: i64,
    service_ptr: *const StringHeader,
    cmd_val: f64,
) -> *mut Promise {
    let promise = js_promise_new();

    let engine = match types::get_compose_engine(handle_id as u64) {
        Some(h) => Arc::clone(h),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    let service_opt = unsafe { string_from_header(service_ptr) };
    let cmd = jsvalue_to_string_array(cmd_val);

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let service = match service_opt {
            Some(s) => s,
            None => return Err("Invalid service name".to_string()),
        };

        match engine.exec(&service, &cmd, None, None).await {
            Ok(logs) => Ok(serde_json::to_string(&logs).unwrap()),
            Err(e) => Err(e.to_string()),
        }
    }, |json| {
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        let jsv = perry_runtime::js_json_parse(str_ptr);
        jsv.bits()
    });

    promise
}

/// Start compose services
/// FFI: js_container_compose_start(handle_id: i64, options: f64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_start(
    handle_id: i64,
    options: f64,
) -> *mut Promise {
    let promise = js_promise_new();
    let engine = match types::get_compose_engine(handle_id as u64) {
        Some(h) => Arc::clone(h),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    let opt_jsv = perry_runtime::JSValue::from_bits(options.to_bits());
    let services = if opt_jsv.is_pointer() {
        let opt_ptr = opt_jsv.as_pointer::<perry_runtime::ObjectHeader>();
        get_object_field_string_array(opt_ptr, "services")
    } else {
        Vec::new()
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match engine.start(&services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });
    promise
}

/// Stop compose services
/// FFI: js_container_compose_stop(handle_id: i64, options: f64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_stop(
    handle_id: i64,
    options: f64,
) -> *mut Promise {
    let promise = js_promise_new();
    let engine = match types::get_compose_engine(handle_id as u64) {
        Some(h) => Arc::clone(h),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    let opt_jsv = perry_runtime::JSValue::from_bits(options.to_bits());
    let services = if opt_jsv.is_pointer() {
        let opt_ptr = opt_jsv.as_pointer::<perry_runtime::ObjectHeader>();
        get_object_field_string_array(opt_ptr, "services")
    } else {
        Vec::new()
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match engine.stop(&services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });
    promise
}

/// Restart compose services
/// FFI: js_container_compose_restart(handle_id: i64, options: f64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_restart(
    handle_id: i64,
    options: f64,
) -> *mut Promise {
    let promise = js_promise_new();
    let engine = match types::get_compose_engine(handle_id as u64) {
        Some(h) => Arc::clone(h),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    let opt_jsv = perry_runtime::JSValue::from_bits(options.to_bits());
    let services = if opt_jsv.is_pointer() {
        let opt_ptr = opt_jsv.as_pointer::<perry_runtime::ObjectHeader>();
        get_object_field_string_array(opt_ptr, "services")
    } else {
        Vec::new()
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match engine.restart(&services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });
    promise
}

/// Get compose configuration
/// FFI: js_container_compose_config(handle_id: i64, options: f64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_config(handle_id: i64, _options: f64) -> *mut Promise {
    let promise = js_promise_new();
    let engine = match types::get_compose_engine(handle_id as u64) {
        Some(h) => Arc::clone(h),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        match engine.config() {
            Ok(yaml) => Ok(yaml),
            Err(e) => Err::<String, String>(e.to_string()),
        }
    }, |yaml| {
        let ptr = perry_runtime::js_string_from_bytes(yaml.as_ptr(), yaml.len() as u32);
        perry_runtime::JSValue::string_ptr(ptr).bits()
    });
    promise
}

// ============ Module Initialization ============

/// Initialize the container module (called during runtime startup)
#[no_mangle]
pub extern "C" fn js_container_module_init() {
}
