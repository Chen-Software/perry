//! Container module for Perry
//!
//! Provides OCI container management with platform-adaptive backend selection.
//! Uses apple/container on macOS/iOS and podman on all other platforms.

pub mod backend;
pub mod compose;
pub mod types;
pub mod verification;

// Re-export commonly used types
pub use types::{
    ComposeHealthcheck, ComposeNetwork, ComposeService, ComposeSpec, ComposeVolume,
    ContainerHandle, ContainerInfo, ContainerLogs, ContainerSpec, ImageInfo,
};

use perry_runtime::{js_promise_new, js_string_from_bytes, Promise, StringHeader, JSValue};
use backend::{get_backend, ContainerBackend};
use std::sync::OnceLock;
use std::sync::Arc;

// Global backend instance - initialized once at first use
static BACKEND: OnceLock<Arc<dyn ContainerBackend>> = OnceLock::new();

/// Get or initialize the global backend instance
fn get_global_backend() -> &'static Arc<dyn ContainerBackend> {
    BACKEND.get_or_init(|| {
        get_backend().expect("Failed to initialize container backend")
    })
}

/// Helper to extract string from StringHeader pointer
unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() || (ptr as usize) < 0x1000 {
        return None;
    }
    let len = (*ptr).length as usize;
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
/// FFI: js_container_run(spec_ptr: *const JSValue) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_run(spec_ptr: *const perry_runtime::JSValue) -> *mut Promise {
    let promise = js_promise_new();
    let backend = Arc::clone(get_global_backend());

    let spec = match types::parse_container_spec(spec_ptr) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(e)
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

/// Create a container from the given spec without starting it
/// FFI: js_container_create(spec_ptr: *const JSValue) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_create(spec_ptr: *const perry_runtime::JSValue) -> *mut Promise {
    let promise = js_promise_new();
    let backend = Arc::clone(get_global_backend());

    let spec = match types::parse_container_spec(spec_ptr) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(e)
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
    let backend = Arc::clone(get_global_backend());

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
    let backend = Arc::clone(get_global_backend());

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
        match backend.stop(&id, timeout as u32).await {
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
    let backend = Arc::clone(get_global_backend());

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
    let backend = Arc::clone(get_global_backend());

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
    let backend = Arc::clone(get_global_backend());

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
    let backend_name = get_global_backend().name();
    string_to_js(backend_name)
}

// ============ Container Logs and Exec ============

/// Get logs from a container
/// FFI: js_container_logs(id_ptr: *const StringHeader, follow: i32, tail: i32) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_logs(id_ptr: *const StringHeader, follow: i32, tail: i32) -> *mut Promise {
    let promise = js_promise_new();
    let backend = Arc::clone(get_global_backend());

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

    // TODO: Implement follow mode with ReadableStream
    if follow != 0 {
        crate::common::spawn_for_promise(promise as *mut u8, async move {
            Err::<u64, String>("Follow mode not yet implemented".to_string())
        });
        return promise;
    }

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
/// FFI: js_container_exec(id_ptr: *const StringHeader, cmd_array: *const JSValue, env_obj: *const JSValue, workdir_ptr: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_exec(
    id_ptr: *const StringHeader,
    _cmd_array: *const JSValue,
    _env_obj: *const JSValue,
    _workdir_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let backend = Arc::clone(get_global_backend());

    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid container ID".to_string())
            });
            return promise;
        }
    };

    // TODO: Parse cmd_array, env_obj, workdir_ptr
    // For now, use empty command
    let cmd = Vec::new();
    let env = None;

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match backend.exec(&id, &cmd, env).await {
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
    let backend = Arc::clone(get_global_backend());

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
    let backend = Arc::clone(get_global_backend());

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
    let backend = Arc::clone(get_global_backend());

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
/// FFI: js_container_composeUp(spec_ptr: *const JSValue) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_composeUp(spec_ptr: *const JSValue) -> *mut Promise {
    let promise = js_promise_new();

    let spec = match types::parse_compose_spec(spec_ptr) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(e)
            });
            return promise;
        }
    };

    let backend = Arc::clone(get_global_backend());
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = compose::ComposeEngine::new(spec, backend);
        match engine.up().await {
            Ok(handle) => {
                let handle_id = types::register_compose_handle(handle);
                Ok(handle_id as u64)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Stop and remove compose stack
/// FFI: js_container_compose_down(handle_id: i64, volumes: i32) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_down(handle_id: i64, volumes: i32) -> *mut Promise {
    let promise = js_promise_new();

    // TODO: Retrieve ComposeHandle from handle_ptr
    // For now, just return success
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        Ok(0u64)
    });

    promise
}

/// Get container info for compose stack
/// FFI: js_composeHandle_ps(handle_ptr: *const JSValue) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_composeHandle_ps(_handle_ptr: *const JSValue) -> *mut Promise {
    let promise = js_promise_new();

    // TODO: Retrieve ComposeHandle from handle_ptr
    // For now, return empty array
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let handle_id = types::register_container_info_list(Vec::new());
        Ok(handle_id as u64)
    });

    promise
}

/// Get logs from compose stack
/// FFI: js_composeHandle_logs(handle_ptr: *const JSValue, service_ptr: *const StringHeader, tail: i32) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_composeHandle_logs(_handle_ptr: *const JSValue, service_ptr: *const StringHeader, tail: i32) -> *mut Promise {
    let promise = js_promise_new();

    let _tail_opt = if tail >= 0 { Some(tail as u32) } else { None };

    // TODO: Retrieve ComposeHandle from handle_ptr
    // For now, return empty logs
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let logs = types::ContainerLogs {
            stdout: String::new(),
            stderr: String::new(),
        };
        let handle_id = types::register_container_logs(logs);
        Ok(handle_id as u64)
    });

    promise
}

/// Execute a command in a compose service
/// FFI: js_composeHandle_exec(handle_ptr: *const JSValue, service_ptr: *const StringHeader, cmd_array: *const JSValue, env_obj: *const JSValue) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_composeHandle_exec(
    _handle_ptr: *const JSValue,
    _service_ptr: *const StringHeader,
    _cmd_array: *const JSValue,
    _env_obj: *const JSValue,
) -> *mut Promise {
    let promise = js_promise_new();

    // TODO: Parse cmd_array and env_obj
    // TODO: Retrieve ComposeHandle from handle_ptr
    // For now, return empty logs
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let logs = types::ContainerLogs {
            stdout: String::new(),
            stderr: String::new(),
        };
        let handle_id = types::register_container_logs(logs);
        Ok(handle_id as u64)
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
