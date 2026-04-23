//! Container module for Perry
//!
//! Provides OCI container management with platform-adaptive backend selection.

pub mod backend;
pub mod capability;
pub mod compose;
pub mod types;
pub mod verification;

pub mod mod_private {
    use super::backend::{detect_backend, ContainerBackend};
    use super::types::ContainerError;
    use std::sync::Arc;
    use std::sync::OnceLock;

    // Global backend instance - initialized once at first use
    pub(crate) static BACKEND: OnceLock<Arc<dyn ContainerBackend>> = OnceLock::new();
    pub(crate) static BACKEND_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    /// Get or initialize the global backend instance using double-checked locking.
    pub async fn get_global_backend() -> Result<Arc<dyn ContainerBackend>, ContainerError> {
        if let Some(b) = BACKEND.get() {
            return Ok(Arc::clone(b));
        }

        let _guard = BACKEND_MUTEX.lock().await;
        if let Some(b) = BACKEND.get() {
            return Ok(Arc::clone(b));
        }

        let b = detect_backend()
            .await
            .map(|b| Arc::from(b) as Arc<dyn ContainerBackend>)
            .map_err(ContainerError::from)?;

        let _ = BACKEND.set(Arc::clone(&b));
        Ok(b)
    }

    pub async fn get_global_backend_instance() -> Result<Arc<dyn ContainerBackend>, ContainerError> {
        get_global_backend().await
    }
}

// Re-export commonly used types
pub use types::{
    ComposeHandle, ComposeSpec, ContainerError, ContainerHandle, ContainerInfo, ContainerLogs,
    ContainerSpec, ImageInfo, ListOrDict,
};

use perry_runtime::{js_promise_new, Promise, StringHeader};
pub use backend::{detect_backend, ContainerBackend};
use std::collections::HashMap;
use std::sync::Arc;

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
                Err::<u64, String>(types::compose_error_to_json(ContainerError::InvalidConfig(e)))
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match mod_private::get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(types::compose_error_to_json(e)),
        };
        match backend.run(&spec).await {
            Ok(handle) => {
                let handle_id = types::register_container_handle(handle);
                Ok(handle_id as u64)
            }
            Err(e) => Err::<u64, String>(types::compose_error_to_json(e.into())),
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
                Err::<u64, String>(types::compose_error_to_json(ContainerError::InvalidConfig(e)))
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match mod_private::get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(types::compose_error_to_json(e)),
        };
        match backend.create(&spec).await {
            Ok(handle) => {
                let handle_id = types::register_container_handle(handle);
                Ok(handle_id as u64)
            }
            Err(e) => Err::<u64, String>(types::compose_error_to_json(e.into())),
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
                Err::<u64, String>(types::compose_error_to_json(ContainerError::InvalidConfig("Invalid container ID".into())))
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match mod_private::get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(types::compose_error_to_json(e)),
        };
        match backend.start(&id).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(types::compose_error_to_json(e.into())),
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
                Err::<u64, String>(types::compose_error_to_json(ContainerError::InvalidConfig("Invalid container ID".into())))
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let timeout_opt = if timeout < 0 { None } else { Some(timeout as u32) };
        let backend = match mod_private::get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(types::compose_error_to_json(e)),
        };
        match backend.stop(&id, timeout_opt).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(types::compose_error_to_json(e.into())),
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
                Err::<u64, String>(types::compose_error_to_json(ContainerError::InvalidConfig("Invalid container ID".into())))
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match mod_private::get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(types::compose_error_to_json(e)),
        };
        match backend.remove(&id, force != 0).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(types::compose_error_to_json(e.into())),
        }
    });

    promise
}

/// List containers
/// FFI: js_container_list(all: i32) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_list(all: i32) -> *mut Promise {
    let promise = js_promise_new();

    crate::common::spawn_for_promise_deferred(
        promise as *mut u8,
        async move {
            let backend = match mod_private::get_global_backend().await {
                Ok(b) => b,
                Err(e) => return Err::<Vec<ContainerInfo>, String>(types::compose_error_to_json(e)),
            };
            match backend.list(all != 0).await {
                Ok(containers) => Ok(containers),
                Err(e) => Err::<Vec<ContainerInfo>, String>(types::compose_error_to_json(e.into())),
            }
        },
        |containers| {
            let handle_id = types::register_container_info_list(containers);
            perry_runtime::JSValue::number(handle_id as f64).bits()
        },
    );

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
                Err::<u64, String>(types::compose_error_to_json(ContainerError::InvalidConfig("Invalid container ID".into())))
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise_deferred(
        promise as *mut u8,
        async move {
            let backend = match mod_private::get_global_backend().await {
                Ok(b) => b,
                Err(e) => return Err::<ContainerInfo, String>(types::compose_error_to_json(e)),
            };
            match backend.inspect(&id).await {
                Ok(info) => Ok(info),
                Err(e) => Err::<ContainerInfo, String>(types::compose_error_to_json(e.into())),
            }
        },
        |info| {
            let handle_id = types::register_container_info(info);
            perry_runtime::JSValue::number(handle_id as f64).bits()
        },
    );

    promise
}

/// Get the current backend name
/// FFI: js_container_getBackend() -> *const StringHeader
#[no_mangle]
pub unsafe extern "C" fn js_container_getBackend() -> *const StringHeader {
    // Note: this is synchronous and might return "unknown" if not initialized
    if let Some(b) = mod_private::BACKEND.get() {
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
                Err::<u64, String>(types::compose_error_to_json(ContainerError::InvalidConfig("Invalid container ID".into())))
            });
            return promise;
        }
    };

    let tail_opt = if tail >= 0 { Some(tail as u32) } else { None };

    crate::common::spawn_for_promise_deferred(
        promise as *mut u8,
        async move {
            let backend = match mod_private::get_global_backend().await {
                Ok(b) => b,
                Err(e) => return Err::<ContainerLogs, String>(types::compose_error_to_json(e)),
            };
            match backend.logs(&id, tail_opt).await {
                Ok(logs) => Ok(logs),
                Err(e) => Err::<ContainerLogs, String>(types::compose_error_to_json(e.into())),
            }
        },
        |logs| {
            let handle_id = types::register_container_logs(logs);
            perry_runtime::JSValue::number(handle_id as f64).bits()
        },
    );

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
                Err::<u64, String>(types::compose_error_to_json(ContainerError::InvalidConfig("Invalid container ID".into())))
            });
            return promise;
        }
    };

    let cmd_json = string_from_header(cmd_json_ptr);
    let env_json = string_from_header(env_json_ptr);
    let workdir = string_from_header(workdir_ptr);

    crate::common::spawn_for_promise_deferred(
        promise as *mut u8,
        async move {
            let cmd: Vec<String> = cmd_json
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();

            let env: Option<HashMap<String, String>> =
                env_json.and_then(|s| serde_json::from_str(&s).ok());

            let backend = match mod_private::get_global_backend().await {
                Ok(b) => b,
                Err(e) => return Err::<ContainerLogs, String>(types::compose_error_to_json(e)),
            };
            match backend.exec(&id, &cmd, env.as_ref(), workdir.as_deref()).await {
                Ok(logs) => Ok(logs),
                Err(e) => Err::<ContainerLogs, String>(types::compose_error_to_json(e.into())),
            }
        },
        |logs| {
            let handle_id = types::register_container_logs(logs);
            perry_runtime::JSValue::number(handle_id as f64).bits()
        },
    );

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
                Err::<u64, String>(types::compose_error_to_json(ContainerError::InvalidConfig("Invalid image reference".into())))
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match mod_private::get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(types::compose_error_to_json(e)),
        };
        match backend.pull_image(&reference).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(types::compose_error_to_json(e.into())),
        }
    });

    promise
}

/// List images
/// FFI: js_container_listImages() -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_listImages() -> *mut Promise {
    let promise = js_promise_new();

    crate::common::spawn_for_promise_deferred(
        promise as *mut u8,
        async move {
            let backend = match mod_private::get_global_backend().await {
                Ok(b) => b,
                Err(e) => return Err::<Vec<ImageInfo>, String>(types::compose_error_to_json(e)),
            };
            match backend.list_images().await {
                Ok(images) => Ok(images),
                Err(e) => Err::<Vec<ImageInfo>, String>(types::compose_error_to_json(e.into())),
            }
        },
        |images| {
            let handle_id = types::register_image_info_list(images);
            perry_runtime::JSValue::number(handle_id as f64).bits()
        },
    );

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
                Err::<u64, String>(types::compose_error_to_json(ContainerError::InvalidConfig("Invalid image reference".into())))
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match mod_private::get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(types::compose_error_to_json(e)),
        };
        match backend.remove_image(&reference, force != 0).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(types::compose_error_to_json(e.into())),
        }
    });

    promise
}

// ============ Compose Functions ============

/// Bring up a Compose stack (alias for js_container_composeUp)
/// FFI: js_container_composeUp(spec_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_composeUp(spec_ptr: *const perry_runtime::StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let spec = match types::parse_compose_spec(spec_ptr) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(types::compose_error_to_json(ContainerError::InvalidConfig(e)))
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise_deferred(
        promise as *mut u8,
        async move {
            match compose::compose_up(spec).await {
                Ok(handle) => Ok(handle),
                Err(e) => Err::<ComposeHandle, String>(e),
            }
        },
        |handle| {
            let handle_id = types::register_compose_handle(handle);
            perry_runtime::JSValue::number(handle_id as f64).bits()
        },
    );

    promise
}

/// Stop and remove compose stack.
/// FFI: js_container_compose_down(handle_id: i64, volumes: i32) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_down(handle_id: i64, volumes: i32) -> *mut Promise {
    let promise = js_promise_new();

    let handle = match types::take_compose_handle(handle_id as u64) {
        Some(h) => h,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(types::compose_error_to_json(ContainerError::InvalidConfig("Invalid compose handle".into())))
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match compose::compose_down(handle.stack_id, volumes != 0).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e),
        }
    });

    promise
}

/// Get container info for compose stack
/// FFI: js_container_compose_ps(handle_id: i64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_ps(handle_id: i64) -> *mut Promise {
    let promise = js_promise_new();

    let handle = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(types::compose_error_to_json(ContainerError::InvalidConfig("Invalid compose handle".into())))
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match compose::compose_ps(handle.stack_id).await {
            Ok(containers) => {
                let h = types::register_container_info_list(containers);
                Ok(h as u64)
            }
            Err(e) => Err::<u64, String>(e),
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

    let handle = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(types::compose_error_to_json(ContainerError::InvalidConfig("Invalid compose handle".into())))
            });
            return promise;
        }
    };

    let service = unsafe { string_from_header(service_ptr) };
    let tail_opt = if tail >= 0 { Some(tail as u32) } else { None };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match compose::compose_logs(handle.stack_id, service, tail_opt).await {
            Ok(logs) => {
                let h = types::register_container_logs(logs);
                Ok(h as u64)
            }
            Err(e) => Err::<u64, String>(e),
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

    let handle = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(types::compose_error_to_json(ContainerError::InvalidConfig("Invalid compose handle".into())))
            });
            return promise;
        }
    };

    let service_opt = unsafe { string_from_header(service_ptr) };
    let cmd_json = unsafe { string_from_header(cmd_json_ptr) };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let service = match service_opt {
            Some(s) => s,
            None => return Err::<u64, String>(types::compose_error_to_json(ContainerError::InvalidConfig("Invalid service name".into()))),
        };

        let cmd: Vec<String> = cmd_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        match compose::compose_exec(handle.stack_id, service, cmd).await {
            Ok(logs) => {
                let h = types::register_container_logs(logs);
                Ok(h as u64)
            }
            Err(e) => Err::<u64, String>(e),
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
            crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
                Err::<String, String>(types::compose_error_to_json(ContainerError::InvalidConfig(e)))
            }, |s| perry_runtime::JSValue::string_ptr(perry_runtime::js_string_from_bytes(s.as_ptr(), s.len() as u32)).bits());
            return promise;
        }
    };

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        compose::compose_config_spec(spec).await
    }, |yaml| {
        let str_ptr = perry_runtime::js_string_from_bytes(yaml.as_ptr(), yaml.len() as u32);
        perry_runtime::JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

/// Start services in compose stack
/// FFI: js_container_compose_start(handle_id: i64, services_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_start(handle_id: i64, services_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let handle = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(types::compose_error_to_json(ContainerError::InvalidConfig("Invalid compose handle".into())))
            });
            return promise;
        }
    };

    let services_json = string_from_header(services_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let services: Vec<String> = services_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        match compose::compose_start(handle.stack_id, services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e),
        }
    });

    promise
}

/// Stop services in compose stack
/// FFI: js_container_compose_stop(handle_id: i64, services_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_stop(
    handle_id: i64,
    services_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();

    let handle = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(types::compose_error_to_json(ContainerError::InvalidConfig("Invalid compose handle".into())))
            });
            return promise;
        }
    };

    let services_json = string_from_header(services_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let services: Vec<String> = services_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        match compose::compose_stop(handle.stack_id, services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e),
        }
    });

    promise
}

/// Restart services in compose stack
/// FFI: js_container_compose_restart(handle_id: i64, services_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_restart(
    handle_id: i64,
    services_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();

    let handle = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(types::compose_error_to_json(ContainerError::InvalidConfig("Invalid compose handle".into())))
            });
            return promise;
        }
    };

    let services_json = string_from_header(services_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let services: Vec<String> = services_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        match compose::compose_restart(handle.stack_id, services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e),
        }
    });

    promise
}

// ============ Module Initialization ============

/// Initialize the container module (called during runtime startup)
#[no_mangle]
pub extern "C" fn js_container_module_init() {
}
