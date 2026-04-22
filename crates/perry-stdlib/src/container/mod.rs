//! Container module for Perry
//!
//! Provides OCI container management with platform-adaptive backend selection.

pub mod backend;
pub mod capability;
pub mod compose;
pub mod types;
pub mod verification;
pub mod workload;

// Re-export commonly used types
pub use types::{
    ComposeHandle, ComposeSpec, ContainerError, ContainerHandle,
    ContainerInfo, ContainerLogs, ContainerSpec, ImageInfo, ListOrDict,
};
pub use perry_container_compose::ComposeEngine;

use perry_runtime::{js_promise_new, Promise, StringHeader};
pub use backend::{detect_backend, ContainerBackend};
use std::sync::OnceLock;
use std::sync::Arc;
use std::collections::HashMap;

// Global backend instance - initialized once at first use
static BACKEND: OnceLock<Arc<dyn ContainerBackend>> = OnceLock::new();
static BACKEND_INIT_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Get or initialize the global backend instance.
/// Uses tokio Mutex for double-checked init to avoid OnceLock sync deadlocks.
pub async fn get_global_backend() -> Result<Arc<dyn ContainerBackend>, ContainerError> {
    if let Some(b) = BACKEND.get() {
        return Ok(Arc::clone(b));
    }

    let _guard = BACKEND_INIT_MUTEX.lock().await;
    if let Some(b) = BACKEND.get() {
        return Ok(Arc::clone(b));
    }

    let b = detect_backend().await
        .map(|b| Arc::from(b) as Arc<dyn ContainerBackend>)
        .map_err(ContainerError::from)?;

    let _ = BACKEND.set(Arc::clone(&b));
    Ok(b)
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
/// FFI: js_container_run(spec_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_run(spec_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

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
        let backend = match crate::container::get_global_backend().await {
            Ok(b) => b,
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
/// FFI: js_container_create(spec_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_create(spec_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

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
        let backend = match crate::container::get_global_backend().await {
            Ok(b) => b,
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
        let backend = match crate::container::get_global_backend().await {
            Ok(b) => b,
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
/// FFI: js_container_stop(id: *const StringHeader, timeout: i32) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_stop(id_ptr: *const StringHeader, timeout: i32) -> *mut Promise {
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
        let timeout_opt = if timeout < 0 { None } else { Some(timeout as u32) };
        let backend = match crate::container::get_global_backend().await {
            Ok(b) => b,
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
/// FFI: js_container_remove(id: *const StringHeader, force: i32) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_remove(id_ptr: *const StringHeader, force: i32) -> *mut Promise {
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
        let backend = match crate::container::get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
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

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match crate::container::get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
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

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match crate::container::get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
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
            Err(e) => {
                let json = if let crate::container::types::ContainerError::NoBackendFound { probed } = crate::container::types::ContainerError::from(e) {
                    serde_json::to_string(&probed).unwrap_or_default()
                } else {
                    "[]".to_string()
                };
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
/// FFI: js_container_logs(id: *const StringHeader, tail: i32) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_logs(id_ptr: *const StringHeader, tail: i32) -> *mut Promise {
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

    let tail_opt = if tail >= 0 { Some(tail as u32) } else { None };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match crate::container::get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
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

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let cmd: Vec<String> = cmd_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let env: Option<HashMap<String, String>> = env_json
            .and_then(|s| serde_json::from_str(&s).ok());

        let backend = match crate::container::get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.exec(&id, &cmd, env.as_ref(), workdir.as_deref()).await {
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
        let backend = match crate::container::get_global_backend().await {
            Ok(b) => b,
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
        let backend = match crate::container::get_global_backend().await {
            Ok(b) => b,
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
/// FFI: js_container_removeImage(reference: *const StringHeader, force: i32) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_removeImage(reference_ptr: *const StringHeader, force: i32) -> *mut Promise {
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
        let backend = match crate::container::get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.remove_image(&reference, force != 0).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

// ============ Compose Functions ============

/// Bring up a Compose stack
/// FFI: js_container_compose_up(spec_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_up(spec_ptr: *const perry_runtime::StringHeader) -> *mut Promise {
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

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend = match crate::container::get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<ComposeHandle, String>(e.to_string()),
        };
        let wrapper = ComposeEngine::new(spec, "default".to_string(), backend);
        match wrapper.up(&[], true, false, false).await {
            Ok(handle) => Ok(handle),
            Err(e) => Err::<ComposeHandle, String>(types::compose_error_to_json(e.into())),
        }
    }, |handle| {
        let handle_id = types::register_compose_handle(handle);
        perry_runtime::JSValue::number(handle_id as f64).bits()
    });

    promise
}

/// Stop and remove compose stack.
/// FFI: js_container_compose_down(handle_id: i64, volumes: i32) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_down(handle_id: i64, volumes: i32) -> *mut Promise {
    let promise = js_promise_new();

    if types::get_compose_handle(handle_id as u64).is_none() {
        crate::common::spawn_for_promise(promise as *mut u8, async move {
            Err::<u64, String>("Invalid compose handle".to_string())
        });
        return promise;
    }

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match crate::container::get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        let wrapper =
            match ComposeEngine::from_registry(handle_id as u64, std::sync::Arc::clone(&backend)) {
                Ok(w) => w,
                Err(e) => return Err::<u64, String>(e.to_string()),
            };
        // Remove from registry on down
        ComposeEngine::unregister(handle_id as u64);
        match wrapper.down(&[], false, volumes != 0).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(types::compose_error_to_json(e.into())),
        }
    });

    promise
}

/// Get container info for compose stack
/// FFI: js_container_compose_ps(handle_id: i64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_ps(handle_id: i64) -> *mut Promise {
    let promise = js_promise_new();

    if types::get_compose_handle(handle_id as u64).is_none() {
        crate::common::spawn_for_promise(promise as *mut u8, async move {
            Err::<u64, String>("Invalid compose handle".to_string())
        });
        return promise;
    }

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match crate::container::get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        let wrapper =
            match ComposeEngine::from_registry(handle_id as u64, std::sync::Arc::clone(&backend)) {
                Ok(w) => w,
                Err(e) => return Err::<u64, String>(e.to_string()),
            };
        match wrapper.ps().await {
            Ok(containers) => {
                let h = types::register_container_info_list(containers);
                Ok(h as u64)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Get logs from compose stack
/// FFI: js_container_compose_logs(handle_id: i64, service: *const StringHeader, tail: i32) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_logs(
    handle_id: i64,
    service_ptr: *const StringHeader,
    tail: i32,
) -> *mut Promise {
    let promise = js_promise_new();

    if types::get_compose_handle(handle_id as u64).is_none() {
        crate::common::spawn_for_promise(promise as *mut u8, async move {
            Err::<u64, String>("Invalid compose handle".to_string())
        });
        return promise;
    }

    let service = unsafe { string_from_header(service_ptr) };
    let tail_opt = if tail >= 0 { Some(tail as u32) } else { None };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match crate::container::get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        let wrapper =
            match ComposeEngine::from_registry(handle_id as u64, std::sync::Arc::clone(&backend)) {
                Ok(w) => w,
                Err(e) => return Err::<u64, String>(e.to_string()),
            };
        match wrapper.logs(service.as_deref(), tail_opt).await {
            Ok(logs) => {
                let h = types::register_container_logs(logs);
                Ok(h as u64)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Execute command in compose service
/// FFI: js_container_compose_exec(handle_id: i64, service: *const StringHeader, cmd_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_exec(
    handle_id: i64,
    service_ptr: *const StringHeader,
    cmd_json_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();

    if types::get_compose_handle(handle_id as u64).is_none() {
        crate::common::spawn_for_promise(promise as *mut u8, async move {
            Err::<u64, String>("Invalid compose handle".to_string())
        });
        return promise;
    }

    let service_opt = unsafe { string_from_header(service_ptr) };
    let cmd_json = unsafe { string_from_header(cmd_json_ptr) };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let service = match service_opt {
            Some(s) => s,
            None => return Err::<u64, String>("Invalid service name".to_string()),
        };

        let cmd: Vec<String> = cmd_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let backend = match crate::container::get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        let wrapper =
            match ComposeEngine::from_registry(handle_id as u64, std::sync::Arc::clone(&backend)) {
                Ok(w) => w,
                Err(e) => return Err::<u64, String>(e.to_string()),
            };
        match wrapper.exec(&service, &cmd).await {
            Ok(logs) => {
                let h = types::register_container_logs(logs);
                Ok(h as u64)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Get resolved configuration for compose stack
/// FFI: js_container_compose_config(spec_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_config(spec_ptr: *const StringHeader) -> *mut Promise {
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

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match crate::container::get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        let wrapper = ComposeEngine::new(spec, "default".to_string(), backend);
        match wrapper.config() {
            Ok(resolved_spec) => {
                let json = resolved_spec;
                let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
                Ok(perry_runtime::JSValue::string_ptr(str_ptr).bits())
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Start services in compose stack
/// FFI: js_container_compose_start(handle_id: i64, services_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_start(handle_id: i64, services_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    if types::get_compose_handle(handle_id as u64).is_none() {
        crate::common::spawn_for_promise(promise as *mut u8, async move {
            Err::<u64, String>("Invalid compose handle".to_string())
        });
        return promise;
    }

    let services_json = unsafe { string_from_header(services_ptr) };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let services: Vec<String> = services_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let backend = match crate::container::get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        let wrapper =
            match ComposeEngine::from_registry(handle_id as u64, std::sync::Arc::clone(&backend)) {
                Ok(w) => w,
                Err(e) => return Err::<u64, String>(e.to_string()),
            };
        match wrapper.start(&services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Stop services in compose stack
/// FFI: js_container_compose_stop(handle_id: i64, services_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_stop(handle_id: i64, services_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    if types::get_compose_handle(handle_id as u64).is_none() {
        crate::common::spawn_for_promise(promise as *mut u8, async move {
            Err::<u64, String>("Invalid compose handle".to_string())
        });
        return promise;
    }

    let services_json = unsafe { string_from_header(services_ptr) };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let services: Vec<String> = services_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let backend = match crate::container::get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        let wrapper =
            match ComposeEngine::from_registry(handle_id as u64, std::sync::Arc::clone(&backend)) {
                Ok(w) => w,
                Err(e) => return Err::<u64, String>(e.to_string()),
            };
        match wrapper.stop(&services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Restart services in compose stack
/// FFI: js_container_compose_restart(handle_id: i64, services_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_restart(handle_id: i64, services_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    if types::get_compose_handle(handle_id as u64).is_none() {
        crate::common::spawn_for_promise(promise as *mut u8, async move {
            Err::<u64, String>("Invalid compose handle".to_string())
        });
        return promise;
    }

    let services_json = unsafe { string_from_header(services_ptr) };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let services: Vec<String> = services_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let backend = match crate::container::get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        let wrapper =
            match ComposeEngine::from_registry(handle_id as u64, std::sync::Arc::clone(&backend)) {
                Ok(w) => w,
                Err(e) => return Err::<u64, String>(e.to_string()),
            };
        match wrapper.restart(&services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

// ============ Workload Graph FFI ============

/// Constructs and serialises a WorkloadGraph
/// FFI: js_workload_graph(name: *const StringHeader, nodes_json: *const StringHeader, edges_json: *const StringHeader) -> *const StringHeader
#[no_mangle]
pub unsafe extern "C" fn js_workload_graph(
    name_ptr: *const StringHeader,
    nodes_ptr: *const StringHeader,
    edges_ptr: *const StringHeader,
) -> *const StringHeader {
    let name = string_from_header(name_ptr).unwrap_or_default();
    let nodes_json = string_from_header(nodes_ptr).unwrap_or_default();
    let edges_json = string_from_header(edges_ptr).unwrap_or_default();

    let nodes = serde_json::from_str(&nodes_json).unwrap_or_default();
    let edges = serde_json::from_str(&edges_json).unwrap_or_default();

    let graph = workload::WorkloadGraph {
        name,
        nodes,
        edges,
    };
    string_to_js(&serde_json::to_string(&graph).unwrap_or_default())
}

/// Run a workload graph
/// FFI: js_workload_runGraph(graph_json: *const StringHeader, opts_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_workload_runGraph(
    graph_ptr: *const StringHeader,
    opts_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let graph_json = string_from_header(graph_ptr).unwrap_or_default();
    let opts_json = string_from_header(opts_ptr).unwrap_or_default();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let graph: workload::WorkloadGraph =
            serde_json::from_str(&graph_json).map_err(|e| e.to_string())?;
        let opts: workload::RunGraphOptions =
            serde_json::from_str(&opts_json).map_err(|e| e.to_string())?;

        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        let engine = workload::WorkloadGraphEngine::new(backend);

        match engine.run(graph, opts).await {
            Ok(handle) => Ok(handle.id),
            Err(e) => Err(e.to_string()),
        }
    });

    promise
}

/// Inspect a workload graph status without starting it
/// FFI: js_workload_inspectGraph(graph_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_workload_inspectGraph(graph_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let graph_json = string_from_header(graph_ptr).unwrap_or_default();

    crate::common::spawn_for_promise_deferred(
        promise as *mut u8,
        async move {
            let graph: workload::WorkloadGraph =
                serde_json::from_str(&graph_json).map_err(|e| e.to_string())?;
            let backend = get_global_backend().await.map_err(|e| e.to_string())?;
            let engine = workload::WorkloadGraphEngine::new(backend);
            engine.inspect(&graph).await.map_err(|e| e.to_string())
        },
        |status| {
            let json = serde_json::to_string(&status).unwrap_or_default();
            let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
            perry_runtime::JSValue::string_ptr(str_ptr).bits()
        },
    );

    promise
}

/// Stop and remove workload graph
/// FFI: js_workload_handle_down(handle_id: i64, opts_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_down(
    handle_id: i64,
    _opts_ptr: *const StringHeader,
) -> *mut Promise {
    // Reuse js_container_compose_down for now as workload handle ID is the stack ID
    js_container_compose_down(handle_id, 1) // default to volumes=true for cleanup
}

/// Get workload graph status
/// FFI: js_workload_handle_status(handle_id: i64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_status(handle_id: i64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise_deferred(
        promise as *mut u8,
        async move {
            let backend = get_global_backend().await.map_err(|e| e.to_string())?;
            let engine = workload::WorkloadGraphEngine::new(backend);
            engine.status(handle_id as u64).await.map_err(|e| e.to_string())
        },
        |status| {
            let json = serde_json::to_string(&status).unwrap_or_default();
            let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
            perry_runtime::JSValue::string_ptr(str_ptr).bits()
        },
    );
    promise
}

/// Get workload graph configuration
/// FFI: js_workload_handle_graph(handle_id: i64) -> *const StringHeader
#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_graph(_handle_id: i64) -> *const StringHeader {
    // Sync return, placeholder
    string_to_js("{}")
}

/// Get logs from a workload node
/// FFI: js_workload_handle_logs(handle_id: i64, node: *const StringHeader, opts_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_logs(
    handle_id: i64,
    node_ptr: *const StringHeader,
    _opts_ptr: *const StringHeader,
) -> *mut Promise {
    js_container_compose_logs(handle_id, node_ptr, -1)
}

/// Execute command in a workload node
/// FFI: js_workload_handle_exec(handle_id: i64, node: *const StringHeader, cmd_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_exec(
    handle_id: i64,
    node_ptr: *const StringHeader,
    cmd_ptr: *const StringHeader,
) -> *mut Promise {
    js_container_compose_exec(handle_id, node_ptr, cmd_ptr)
}

/// List nodes in a workload graph
/// FFI: js_workload_handle_ps(handle_id: i64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_ps(handle_id: i64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match crate::container::get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        let wrapper =
            match ComposeEngine::from_registry(handle_id as u64, std::sync::Arc::clone(&backend)) {
                Ok(w) => w,
                Err(e) => return Err::<u64, String>(e.to_string()),
            };
        match wrapper.ps().await {
            Ok(containers) => {
                let h = types::register_container_info_list(containers);
                Ok(h as u64)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });
    promise
}

// ============ Module Initialization ============

/// Initialize the container module (called during runtime startup)
#[no_mangle]
pub extern "C" fn js_container_module_init() {
    // Trigger lazy init of backend
    crate::common::RUNTIME.spawn(async {
        let _ = get_global_backend().await;
    });
}
