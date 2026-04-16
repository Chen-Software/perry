//! Container module for Perry
//!
//! Provides OCI container management with platform-adaptive backend selection.

pub mod backend;
pub mod capability;
pub mod compose;
pub mod types;
pub mod verification;

use perry_runtime::{js_promise_new, Promise, StringHeader, JSValue};
use backend::{detect_backend, ContainerBackend};
use std::sync::Arc;
use once_cell::sync::OnceCell;
pub use types::*;

// Global backend instance - initialized once at first use
static BACKEND: OnceCell<Arc<dyn ContainerBackend>> = OnceCell::new();

/// Get or initialize the global backend instance
pub fn get_global_backend() -> Result<Arc<dyn ContainerBackend>, ContainerError> {
    BACKEND.get_or_try_init(|| {
        tokio::runtime::Handle::current().block_on(async {
            detect_backend().await.map(Arc::from).map_err(ContainerError::from)
        })
    }).cloned()
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

/// Run a container from the given spec JSON
/// FFI: js_container_run(spec_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_run(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid spec JSON".to_string())
            });
            return promise;
        }
    };

    let spec: ContainerSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            let msg = format!("Invalid ContainerSpec: {}", e);
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
            });
            return promise;
        }
    };

    let backend = match get_global_backend() {
        Ok(b) => b,
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
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

/// Create a container from the given spec JSON without starting it
/// FFI: js_container_create(spec_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_create(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid spec JSON".to_string())
            });
            return promise;
        }
    };

    let spec: ContainerSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            let msg = format!("Invalid ContainerSpec: {}", e);
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
            });
            return promise;
        }
    };

    let backend = match get_global_backend() {
        Ok(b) => b,
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
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
/// FFI: js_container_start(id_ptr: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_start(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let backend = match get_global_backend() {
        Ok(b) => b,
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
            });
            return promise;
        }
    };

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
        match backend.start(&id).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Stop a running container
/// FFI: js_container_stop(id_ptr: *const StringHeader, timeout: i32) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_stop(id_ptr: *const StringHeader, timeout: i32) -> *mut Promise {
    let promise = js_promise_new();
    let backend = match get_global_backend() {
        Ok(b) => b,
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
            });
            return promise;
        }
    };

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
        let timeout_opt = if timeout < 0 { None } else { Some(timeout as u32) };
        match backend.stop(&id, timeout_opt).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Remove a container
/// FFI: js_container_remove(id_ptr: *const StringHeader, force: i32) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_remove(id_ptr: *const StringHeader, force: i32) -> *mut Promise {
    let promise = js_promise_new();
    let backend = match get_global_backend() {
        Ok(b) => b,
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
            });
            return promise;
        }
    };

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
        match backend.remove(&id, force != 0).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// List containers
/// FFI: js_container_list(all: i32) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_list(all: i32) -> *mut Promise {
    let promise = js_promise_new();
    let backend = match get_global_backend() {
        Ok(b) => b,
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match backend.list(all != 0).await {
            Ok(containers) => {
                let handle_id = types::register_container_info_list(containers);
                Ok(handle_id as u64)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Inspect a container
/// FFI: js_container_inspect(id_ptr: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_inspect(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let backend = match get_global_backend() {
        Ok(b) => b,
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
            });
            return promise;
        }
    };

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
        match backend.inspect(&id).await {
            Ok(info) => {
                let handle_id = types::register_container_info(info);
                Ok(handle_id as u64)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Get the current backend name
/// FFI: js_container_getBackend() -> *const StringHeader
#[no_mangle]
pub unsafe extern "C" fn js_container_getBackend() -> *const StringHeader {
    match get_global_backend() {
        Ok(b) => string_to_js(b.backend_name()),
        Err(_) => string_to_js("none"),
    }
}

// ============ Container Logs and Exec ============

/// Get logs from a container
/// FFI: js_container_logs(id_ptr: *const StringHeader, tail: i32) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_logs(id_ptr: *const StringHeader, tail: i32) -> *mut Promise {
    let promise = js_promise_new();
    let backend = match get_global_backend() {
        Ok(b) => b,
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
            });
            return promise;
        }
    };

    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid container ID".to_string())
            });
            return promise;
        }
    };

    let tail_opt = if tail >= 0 { Some(tail as u32) } else { None };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match backend.logs(&id, tail_opt).await {
            Ok(logs) => {
                let handle_id = types::register_container_logs(logs);
                Ok(handle_id as u64)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Execute a command in a container
/// FFI: js_container_exec(id_ptr: *const StringHeader, cmd_json: *const StringHeader, env_json: *const StringHeader, workdir: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_exec(
    id_ptr: *const StringHeader,
    cmd_json_ptr: *const StringHeader,
    env_json_ptr: *const StringHeader,
    workdir_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let backend = match get_global_backend() {
        Ok(b) => b,
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
            });
            return promise;
        }
    };

    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid container ID".to_string())
            });
            return promise;
        }
    };

    let cmd_json = match string_from_header(cmd_json_ptr) {
        Some(s) => s,
        None => "[]".to_string(),
    };
    let cmd: Vec<String> = serde_json::from_str(&cmd_json).unwrap_or_default();

    let env_json = match string_from_header(env_json_ptr) {
        Some(s) => s,
        None => "{}".to_string(),
    };
    let env: std::collections::HashMap<String, String> = serde_json::from_str(&env_json).unwrap_or_default();

    let workdir = string_from_header(workdir_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let env_opt = if env.is_empty() { None } else { Some(&env) };
        match backend.exec(&id, &cmd, env_opt, workdir.as_deref()).await {
            Ok(logs) => {
                let handle_id = types::register_container_logs(logs);
                Ok(handle_id as u64)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

// ============ Image Management ============

/// Pull a container image
/// FFI: js_container_pullImage(reference_ptr: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_pullImage(reference_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let backend = match get_global_backend() {
        Ok(b) => b,
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
            });
            return promise;
        }
    };

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
    let backend = match get_global_backend() {
        Ok(b) => b,
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
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
/// FFI: js_container_removeImage(reference_ptr: *const StringHeader, force: i32) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_removeImage(reference_ptr: *const StringHeader, force: i32) -> *mut Promise {
    let promise = js_promise_new();
    let backend = match get_global_backend() {
        Ok(b) => b,
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
            });
            return promise;
        }
    };

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
        match backend.remove_image(&reference, force != 0).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

// ============ Compose Functions ============

/// Bring up a Compose stack
/// FFI: js_container_composeUp(spec_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_up(spec_json_ptr: *const StringHeader) -> *mut Promise { js_container_composeUp(spec_json_ptr) }

#[no_mangle]
pub unsafe extern "C" fn js_container_composeUp(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid spec JSON".to_string())
            });
            return promise;
        }
    };

    let spec: ComposeSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            let msg = format!("Invalid ComposeSpec: {}", e);
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
            });
            return promise;
        }
    };

    let backend = match get_global_backend() {
        Ok(b) => b,
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match compose::compose_up(spec, backend).await {
            Ok(handle) => {
                let handle_id = types::register_compose_handle(handle);
                Ok(handle_id as u64)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Start services in a compose stack
/// FFI: js_container_compose_start(handle_id: i64, services_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_start(handle_id: i64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let services_json = string_from_header(services_json_ptr).unwrap_or_else(|| "[]".to_string());
    let services: Vec<String> = serde_json::from_str(&services_json).unwrap_or_default();

    let engine = match perry_container_compose::ComposeEngine::get_engine(handle_id as u64) {
        Some(e) => e,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match engine.start(&services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Stop services in a compose stack
/// FFI: js_container_compose_stop(handle_id: i64, services_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_stop(handle_id: i64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let services_json = string_from_header(services_json_ptr).unwrap_or_else(|| "[]".to_string());
    let services: Vec<String> = serde_json::from_str(&services_json).unwrap_or_default();

    let engine = match perry_container_compose::ComposeEngine::get_engine(handle_id as u64) {
        Some(e) => e,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match engine.stop(&services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Restart services in a compose stack
/// FFI: js_container_compose_restart(handle_id: i64, services_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_restart(handle_id: i64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let services_json = string_from_header(services_json_ptr).unwrap_or_else(|| "[]".to_string());
    let services: Vec<String> = serde_json::from_str(&services_json).unwrap_or_default();

    let engine = match perry_container_compose::ComposeEngine::get_engine(handle_id as u64) {
        Some(e) => e,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match engine.restart(&services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Stop and remove compose stack.
///
/// `handle_id` is the u64 handle returned by `composeUp()`.
/// `volumes` flag controls whether to remove volumes too.
/// FFI: js_container_compose_down(handle_id: i64, volumes: i32) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_down(handle_id: i64, volumes: i32) -> *mut Promise {
    let promise = js_promise_new();

    let engine = match perry_container_compose::ComposeEngine::get_engine(handle_id as u64) {
        Some(e) => e,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match engine.down(&[], false, volumes != 0).await {
            Ok(()) => {
                perry_container_compose::ComposeEngine::unregister(handle_id as u64);
                Ok(0u64)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Get container info for all services in the compose stack.
/// FFI: js_container_compose_ps(handle_id: i64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_ps(handle_id: i64) -> *mut Promise {
    let promise = js_promise_new();

    let engine = match perry_container_compose::ComposeEngine::get_engine(handle_id as u64) {
        Some(e) => e,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match engine.ps().await {
            Ok(containers) => {
                let h = types::register_container_info_list(containers);
                Ok(h as u64)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Get logs from compose stack.
///
/// `service_ptr` can be null for all services.
/// `tail` < 0 means no tail limit.
/// FFI: js_container_compose_logs(handle_id: i64, service_ptr: *const StringHeader, tail: i32) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_logs(
    handle_id: i64,
    service_ptr: *const StringHeader,
    tail: i32,
) -> *mut Promise {
    let promise = js_promise_new();

    let engine = match perry_container_compose::ComposeEngine::get_engine(handle_id as u64) {
        Some(e) => e,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    let service = string_from_header(service_ptr);
    let tail_opt = if tail >= 0 { Some(tail as u32) } else { None };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let services = match service {
            Some(s) => vec![s],
            None => vec![],
        };
        match engine.logs(&services, tail_opt).await {
            Ok(logs) => {
                let h = types::register_container_logs(logs);
                Ok(h as u64)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Execute a command in a compose service.
///
/// `cmd_json_ptr` is a JSON array of strings.
/// FFI: js_container_compose_exec(handle_id: i64, service_ptr: *const StringHeader, cmd_json_ptr: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_exec(
    handle_id: i64,
    service_ptr: *const StringHeader,
    cmd_json_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();

    let engine = match perry_container_compose::ComposeEngine::get_engine(handle_id as u64) {
        Some(e) => e,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    let service = match string_from_header(service_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid service name".to_string())
            });
            return promise;
        }
    };

    let cmd_json = match string_from_header(cmd_json_ptr) {
        Some(s) => s,
        None => "[]".to_string(),
    };
    let cmd: Vec<String> = serde_json::from_str(&cmd_json).unwrap_or_default();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match engine.exec(&service, &cmd).await {
            Ok(logs) => {
                let h = types::register_container_logs(logs);
                Ok(h as u64)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Validate and return the resolved compose configuration
/// FFI: js_container_compose_config(spec_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_config(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid spec JSON".to_string())
            });
            return promise;
        }
    };

    let spec: ComposeSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            let msg = format!("Invalid ComposeSpec: {}", e);
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match serde_yaml::to_string(&spec) {
            Ok(yaml) => {
                let bytes = yaml.as_bytes();
                let ptr = perry_runtime::js_string_from_bytes(bytes.as_ptr(), bytes.len() as u32);
                Ok(ptr as u64)
            }
            Err(e) => Err::<u64, String>(format!("YAML serialization failed: {}", e)),
        }
    });

    promise
}

// ============ Module Initialization ============

/// Initialize the container module (called during runtime startup)
#[no_mangle]
pub extern "C" fn js_container_module_init() {
    // Force backend initialization
    let _ = get_global_backend();
}
