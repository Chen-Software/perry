//! Container module for Perry
//!
//! Provides OCI container management and multi-container orchestration
//! with platform-adaptive backend selection.

pub mod backend;
pub mod capability;
pub mod compose;
pub mod types;
pub mod verification;

use perry_container_compose::error::{compose_error_to_js, ComposeError};
use perry_runtime::{js_promise_new, Promise, StringHeader};
pub use backend::{detect_backend, ContainerBackend};
use std::sync::OnceLock;
use std::sync::Arc;
use std::collections::HashMap;
use std::future::Future;

// Global backend instance - initialized once at first use
static BACKEND: OnceLock<Arc<dyn ContainerBackend>> = OnceLock::new();

/// Get or initialize the global backend instance
async fn get_global_backend() -> Result<&'static Arc<dyn ContainerBackend>, ComposeError> {
    if let Some(b) = BACKEND.get() {
        return Ok(b);
    }

    let b = detect_backend().await
        .map(|b| Arc::from(b) as Arc<dyn ContainerBackend>)
        .map_err(|probed| ComposeError::NoBackendFound { probed })?;

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

/// Spawn an async operation and handle ComposeError by converting it to a JSON error object.
unsafe fn spawn_container_promise<T, F, C>(promise: *mut Promise, future: F, converter: C)
where
    T: Send + 'static,
    F: Future<Output = Result<T, ComposeError>> + Send + 'static,
    C: FnOnce(T) -> u64 + Send + 'static,
{
    crate::common::async_bridge::spawn_for_promise_deferred(promise as *mut u8, async move {
        future.await.map_err(|e| compose_error_to_js(&e))
    }, converter);
}

// ============ Container Lifecycle ============

/// Run a container from the given spec JSON string
/// FFI: js_container_run(spec_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_run(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(compose_error_to_js(&ComposeError::validation("Invalid spec JSON")))
            });
            return promise;
        }
    };

    let spec: types::ContainerSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(compose_error_to_js(&ComposeError::JsonError(e)))
            });
            return promise;
        }
    };

    spawn_container_promise(promise, async move {
        let backend = get_global_backend().await?;
        backend.run(&spec).await
    }, |handle| {
        types::register_container_handle(handle) as u64
    });

    promise
}

/// Create a container from the given spec JSON string without starting it
/// FFI: js_container_create(spec_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_create(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(compose_error_to_js(&ComposeError::validation("Invalid spec JSON")))
            });
            return promise;
        }
    };

    let spec: types::ContainerSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(compose_error_to_js(&ComposeError::JsonError(e)))
            });
            return promise;
        }
    };

    spawn_container_promise(promise, async move {
        let backend = get_global_backend().await?;
        backend.create(&spec).await
    }, |handle| {
        types::register_container_handle(handle) as u64
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
                Err::<u64, String>(compose_error_to_js(&ComposeError::validation("Invalid container ID")))
            });
            return promise;
        }
    };

    spawn_container_promise(promise, async move {
        let backend = get_global_backend().await?;
        backend.start(&id).await?;
        Ok(())
    }, |_| 0u64);

    promise
}

/// Stop a running container
/// FFI: js_container_stop(id: *const StringHeader, timeout: i64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_stop(id_ptr: *const StringHeader, timeout: i64) -> *mut Promise {
    let promise = js_promise_new();

    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(compose_error_to_js(&ComposeError::validation("Invalid container ID")))
            });
            return promise;
        }
    };

    spawn_container_promise(promise, async move {
        let timeout_opt = if timeout < 0 { None } else { Some(timeout as u32) };
        let backend = get_global_backend().await?;
        backend.stop(&id, timeout_opt).await?;
        Ok(())
    }, |_| 0u64);

    promise
}

/// Remove a container
/// FFI: js_container_remove(id: *const StringHeader, force: i64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_remove(id_ptr: *const StringHeader, force: i64) -> *mut Promise {
    let promise = js_promise_new();

    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(compose_error_to_js(&ComposeError::validation("Invalid container ID")))
            });
            return promise;
        }
    };

    spawn_container_promise(promise, async move {
        let backend = get_global_backend().await?;
        backend.remove(&id, force != 0).await?;
        Ok(())
    }, |_| 0u64);

    promise
}

/// List containers
/// FFI: js_container_list(all: i64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_list(all: i64) -> *mut Promise {
    let promise = js_promise_new();

    spawn_container_promise(promise, async move {
        let backend = get_global_backend().await?;
        backend.list(all != 0).await
    }, |containers| {
        types::register_container_info_list(containers) as u64
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
                Err::<u64, String>(compose_error_to_js(&ComposeError::validation("Invalid container ID")))
            });
            return promise;
        }
    };

    spawn_container_promise(promise, async move {
        let backend = get_global_backend().await?;
        backend.inspect(&id).await
    }, |info| {
        types::register_container_info(info) as u64
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

// ============ Container Logs and Exec ============

/// Get logs from a container
/// FFI: js_container_logs(id: *const StringHeader, tail: i64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_logs(id_ptr: *const StringHeader, tail: i64) -> *mut Promise {
    let promise = js_promise_new();

    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(compose_error_to_js(&ComposeError::validation("Invalid container ID")))
            });
            return promise;
        }
    };

    let tail_opt = if tail >= 0 { Some(tail as u32) } else { None };

    spawn_container_promise(promise, async move {
        let backend = get_global_backend().await?;
        backend.logs(&id, tail_opt).await
    }, |logs| {
        types::register_container_logs(logs) as u64
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
                Err::<u64, String>(compose_error_to_js(&ComposeError::validation("Invalid container ID")))
            });
            return promise;
        }
    };

    let cmd_json = string_from_header(cmd_json_ptr);
    let env_json = string_from_header(env_json_ptr);
    let workdir = string_from_header(workdir_ptr);

    spawn_container_promise(promise, async move {
        let cmd: Vec<String> = cmd_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let env: Option<HashMap<String, String>> = env_json
            .and_then(|s| serde_json::from_str(&s).ok());

        let backend = get_global_backend().await?;
        backend.exec(&id, &cmd, env.as_ref(), workdir.as_deref()).await
    }, |logs| {
        types::register_container_logs(logs) as u64
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
                Err::<u64, String>(compose_error_to_js(&ComposeError::validation("Invalid image reference")))
            });
            return promise;
        }
    };

    spawn_container_promise(promise, async move {
        let backend = get_global_backend().await?;
        backend.pull_image(&reference).await?;
        Ok(())
    }, |_| 0u64);

    promise
}

/// List images
/// FFI: js_container_listImages() -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_listImages() -> *mut Promise {
    let promise = js_promise_new();

    spawn_container_promise(promise, async move {
        let backend = get_global_backend().await?;
        backend.list_images().await
    }, |images| {
        types::register_image_info_list(images) as u64
    });

    promise
}

/// Remove an image
/// FFI: js_container_removeImage(reference: *const StringHeader, force: i64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_removeImage(reference_ptr: *const StringHeader, force: i64) -> *mut Promise {
    let promise = js_promise_new();

    let reference = match string_from_header(reference_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(compose_error_to_js(&ComposeError::validation("Invalid image reference")))
            });
            return promise;
        }
    };

    spawn_container_promise(promise, async move {
        let backend = get_global_backend().await?;
        backend.remove_image(&reference, force != 0).await?;
        Ok(())
    }, |_| 0u64);

    promise
}

// ============ Compose Functions ============

/// Bring up a Compose stack
/// FFI: js_compose_up(spec_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_compose_up(spec_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let spec_json = match string_from_header(spec_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(compose_error_to_js(&ComposeError::validation("Invalid spec JSON")))
            });
            return promise;
        }
    };

    let spec: types::ComposeSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(compose_error_to_js(&ComposeError::JsonError(e)))
            });
            return promise;
        }
    };

    spawn_container_promise(promise, async move {
        let backend = get_global_backend().await?;
        compose::compose_up(spec, Arc::clone(backend)).await.map_err(|e| ComposeError::ValidationError { message: e.to_string() })
    }, |engine| {
        types::register_compose_engine((*engine).clone()) as u64
    });

    promise
}

/// Stop and remove compose stack.
/// FFI: js_compose_down(handle_id: i64, volumes: i64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_compose_down(handle_id: i64, volumes: i64) -> *mut Promise {
    let promise = js_promise_new();

    spawn_container_promise(promise, async move {
        let engine = types::take_compose_engine(handle_id as u64)
            .ok_or_else(|| ComposeError::validation("Invalid compose handle"))?;
        engine.down(&[], false, volumes != 0).await?;
        Ok(())
    }, |_| 0u64);

    promise
}

/// Get container info for compose stack
/// FFI: js_compose_ps(handle_id: i64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_compose_ps(handle_id: i64) -> *mut Promise {
    let promise = js_promise_new();

    spawn_container_promise(promise, async move {
        let engine = types::get_compose_engine_arc(handle_id as u64)
            .ok_or_else(|| ComposeError::validation("Invalid compose handle"))?;

        engine.ps().await
    }, |containers| {
        types::register_container_info_list(containers) as u64
    });

    promise
}

/// Get logs from compose stack
/// FFI: js_compose_logs(handle_id: i64, service: *const StringHeader, tail: i64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_compose_logs(
    handle_id: i64,
    service_ptr: *const StringHeader,
    tail: i64,
) -> *mut Promise {
    let promise = js_promise_new();

    let service = unsafe { string_from_header(service_ptr) };
    let tail_opt = if tail >= 0 { Some(tail as u32) } else { None };

    spawn_container_promise(promise, async move {
        let engine = types::get_compose_engine_arc(handle_id as u64)
            .ok_or_else(|| ComposeError::validation("Invalid compose handle"))?;

        let services: Vec<String> = service.map(|s| vec![s]).unwrap_or_default();
        let map = engine.logs(&services, tail_opt).await?;
        let stdout = map.values().cloned().collect::<Vec<_>>().join("\n");
        Ok(types::ContainerLogs { stdout, stderr: String::new() })
    }, |logs| {
        types::register_container_logs(logs) as u64
    });

    promise
}

/// Execute command in compose service
/// FFI: js_compose_exec(handle_id: i64, service: *const StringHeader, cmd_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_compose_exec(
    handle_id: i64,
    service_ptr: *const StringHeader,
    cmd_json_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();

    let service_opt = unsafe { string_from_header(service_ptr) };
    let cmd_json = unsafe { string_from_header(cmd_json_ptr) };

    spawn_container_promise(promise, async move {
        let service = service_opt.ok_or_else(|| ComposeError::validation("Invalid service name"))?;

        let cmd: Vec<String> = cmd_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let engine = types::get_compose_engine_arc(handle_id as u64)
            .ok_or_else(|| ComposeError::validation("Invalid compose handle"))?;

        engine.exec(&service, &cmd, None, None).await
    }, |logs| {
        types::register_container_logs(logs) as u64
    });

    promise
}

/// Validate and print resolved configuration
/// FFI: js_compose_config(spec_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_compose_config(spec_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let spec_json = match string_from_header(spec_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(compose_error_to_js(&ComposeError::validation("Invalid spec JSON")))
            });
            return promise;
        }
    };

    let spec: types::ComposeSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(compose_error_to_js(&ComposeError::JsonError(e)))
            });
            return promise;
        }
    };

    spawn_container_promise(promise, async move {
        let backend = get_global_backend().await?;
        let wrapper = compose::ComposeWrapper::new(spec, Arc::clone(backend));
        wrapper.engine.config()
    }, |config_yaml| {
        let str_ptr = perry_runtime::js_string_from_bytes(config_yaml.as_ptr(), config_yaml.len() as u32);
        // Box as string handle
        perry_runtime::JSValue::string_ptr(str_ptr).bits()
    });

    promise
}

/// Start existing stopped services
/// FFI: js_compose_start(handle_id: i64, services_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_compose_start(handle_id: i64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let services_json = unsafe { string_from_header(services_json_ptr) };

    spawn_container_promise(promise, async move {
        let services: Vec<String> = services_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let engine = types::get_compose_engine_arc(handle_id as u64)
            .ok_or_else(|| ComposeError::validation("Invalid compose handle"))?;

        engine.start(&services).await?;
        Ok(())
    }, |_| 0u64);

    promise
}

/// Stop running services
/// FFI: js_compose_stop(handle_id: i64, services_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_compose_stop(handle_id: i64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let services_json = unsafe { string_from_header(services_json_ptr) };

    spawn_container_promise(promise, async move {
        let services: Vec<String> = services_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let engine = types::get_compose_engine_arc(handle_id as u64)
            .ok_or_else(|| ComposeError::validation("Invalid compose handle"))?;

        engine.stop(&services).await?;
        Ok(())
    }, |_| 0u64);

    promise
}

/// Restart services
/// FFI: js_compose_restart(handle_id: i64, services_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_compose_restart(handle_id: i64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let services_json = unsafe { string_from_header(services_json_ptr) };

    spawn_container_promise(promise, async move {
        let services: Vec<String> = services_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let engine = types::get_compose_engine_arc(handle_id as u64)
            .ok_or_else(|| ComposeError::validation("Invalid compose handle"))?;

        engine.restart(&services).await?;
        Ok(())
    }, |_| 0u64);

    promise
}

/// Compatibility alias for composeUp
#[no_mangle]
pub unsafe extern "C" fn js_container_composeUp(spec_ptr: *const StringHeader) -> *mut Promise {
    js_compose_up(spec_ptr)
}

// ============ Module Initialization ============

/// Initialize the container module (called during runtime startup)
#[no_mangle]
pub extern "C" fn js_container_module_init() {
}
