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
pub use backend::{detect_backend, ContainerBackend};
use perry_runtime::{js_promise_new, Promise, StringHeader};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::OnceLock;
pub use types::{
    ComposeHandle, ComposeSpec, ContainerError, ContainerHandle, ContainerInfo, ContainerLogs,
    ContainerSpec, ImageInfo, ListOrDict,
};
use tokio::sync::Mutex;

// Global backend instance - initialized once at first use
static BACKEND: OnceLock<Arc<dyn ContainerBackend>> = OnceLock::new();
static BACKEND_INIT_MUTEX: Mutex<()> = Mutex::const_new(());

/// Get or initialize the global backend instance (async)
pub async fn get_global_backend_instance_async() -> Result<Arc<dyn ContainerBackend>, ContainerError> {
    if let Some(b) = BACKEND.get() {
        return Ok(Arc::clone(b));
    }

    let _guard = BACKEND_INIT_MUTEX.lock().await;
    if let Some(b) = BACKEND.get() {
        return Ok(Arc::clone(b));
    }

    let b = match detect_backend().await {
        Ok(b) => b,
        Err(probed) => {
            // Try interactive installer if no backend found
            match perry_container_compose::installer::BackendInstaller::run().await {
                Ok(installed_backend) => installed_backend,
                Err(_) => return Err(ContainerError::NoBackendFound { probed }),
            }
        }
    };

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
unsafe fn string_to_js(s: &str) -> *mut StringHeader {
    let bytes = s.as_bytes();
    perry_runtime::js_string_from_bytes(bytes.as_ptr(), bytes.len() as u32) as *mut StringHeader
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
        let backend = get_global_backend_instance_async()
            .await
            .map_err(|e| e.to_string())?;
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

/// Check if an image exists locally
/// FFI: js_container_imageExists(reference: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_imageExists(
    reference_ptr: *const StringHeader,
) -> *mut Promise {
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
        let backend = get_global_backend_instance_async()
            .await
            .map_err(|e| e.to_string())?;

        match backend.inspect_image(&reference).await {
            Ok(_) => Ok(1u64),  // true
            Err(_) => Ok(0u64), // false
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
        let backend = get_global_backend_instance_async()
            .await
            .map_err(|e| e.to_string())?;
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
        let backend = get_global_backend_instance_async()
            .await
            .map_err(|e| e.to_string())?;
        match backend.start(&id).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Stop a running container
/// FFI: js_container_stop(id: *const StringHeader, opts_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_stop(
    id_ptr: *const StringHeader,
    opts_ptr: *const StringHeader,
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

    let opts_json = string_from_header(opts_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let timeout = opts_json
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v.get("timeout").and_then(|t| t.as_u64()).map(|t| t as u32));

        let backend = get_global_backend_instance_async()
            .await
            .map_err(|e| e.to_string())?;
        match backend.stop(&id, timeout).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Remove a container
/// FFI: js_container_remove(id: *const StringHeader, opts_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_remove(
    id_ptr: *const StringHeader,
    opts_ptr: *const StringHeader,
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

    let opts_json = string_from_header(opts_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let force = opts_json
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v.get("force").and_then(|f| f.as_bool()))
            .unwrap_or(false);

        let backend = get_global_backend_instance_async()
            .await
            .map_err(|e| e.to_string())?;
        match backend.remove(&id, force).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// List containers
/// FFI: js_container_list(opts_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_list(opts_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let opts_json = string_from_header(opts_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let all = opts_json
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v.get("all").and_then(|a| a.as_bool()))
            .unwrap_or(false);

        let backend = get_global_backend_instance_async()
            .await
            .map_err(|e| e.to_string())?;
        match backend.list(all).await {
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
        let backend = get_global_backend_instance_async()
            .await
            .map_err(|e| e.to_string())?;
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

/// Get the current backend name (synchronous)
/// FFI: js_container_getBackend() -> *const StringHeader
#[no_mangle]
pub unsafe extern "C" fn js_container_getBackend() -> *const StringHeader {
    let name = BACKEND
        .get()
        .map(|b| b.backend_name())
        .unwrap_or("unknown");
    string_to_js(name)
}

/// Detect backend and return probed info
/// FFI: js_container_detectBackend() -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_detectBackend() -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise_deferred(
        promise as *mut u8,
        async move {
            match detect_backend().await {
                Ok(b) => {
                    let name = b.backend_name().to_string();
                    let json = serde_json::json!([{
                        "name": name,
                        "available": true,
                        "reason": ""
                    }])
                    .to_string();

                    // Cache it if not already set
                    let _ = BACKEND.set(Arc::clone(&b));

                    Ok(json)
                }
                Err(probed) => {
                    let json = serde_json::to_string(&probed).unwrap_or_default();
                    Ok(json) // Resolve with probe info array on failure to find any
                }
            }
        },
        |json| {
            let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
            perry_runtime::JSValue::string_ptr(str_ptr).bits()
        },
    );
    promise
}

// ============ Container Logs and Exec ============

/// Get logs from a container
/// FFI: js_container_logs(id: *const StringHeader, opts_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_logs(
    id_ptr: *const StringHeader,
    opts_ptr: *const StringHeader,
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

    let opts_json = string_from_header(opts_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let tail = opts_json
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v.get("tail").and_then(|t| t.as_u64()).map(|t| t as u32));

        let backend = get_global_backend_instance_async()
            .await
            .map_err(|e| e.to_string())?;
        match backend.logs(&id, tail).await {
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

        let env: Option<HashMap<String, String>> =
            env_json.and_then(|s| serde_json::from_str(&s).ok());

        let backend = get_global_backend_instance_async()
            .await
            .map_err(|e| e.to_string())?;
        match backend
            .exec(&id, &cmd, env.as_ref(), workdir.as_deref())
            .await
        {
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
        let backend = get_global_backend_instance_async()
            .await
            .map_err(|e| e.to_string())?;
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
        let backend = get_global_backend_instance_async()
            .await
            .map_err(|e| e.to_string())?;
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
pub unsafe extern "C" fn js_container_removeImage(
    reference_ptr: *const StringHeader,
    force: i32,
) -> *mut Promise {
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
        let backend = get_global_backend_instance_async()
            .await
            .map_err(|e| e.to_string())?;
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
pub unsafe extern "C" fn js_container_compose_up(spec_ptr: *const StringHeader) -> *mut Promise {
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
        let backend = get_global_backend_instance_async()
            .await
            .map_err(|e| e.to_string())?;
        let wrapper = compose::ComposeWrapper::new(spec, backend);
        match wrapper.up().await {
            Ok(handle) => {
                let handle_id = types::register_compose_handle(handle);
                Ok(handle_id as u64)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Stop and remove compose stack.
/// FFI: js_container_compose_down(handle_id: i64, opts_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_down(handle_id: i64, opts_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let handle = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    let opts_json = string_from_header(opts_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let volumes = opts_json
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v.get("volumes").and_then(|vol| vol.as_bool()))
            .unwrap_or(false);

        let backend = get_global_backend_instance_async()
            .await
            .map_err(|e| e.to_string())?;
        let wrapper = match compose::ComposeWrapper::new_with_handle(&handle, backend) {
            Ok(w) => w,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match wrapper.down(&handle, volumes).await {
            Ok(()) => {
                types::take_compose_handle(handle_id as u64);
                Ok(0u64)
            },
            Err(e) => Err::<u64, String>(e.to_string()),
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
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance_async()
            .await
            .map_err(|e| e.to_string())?;
        let wrapper = match compose::ComposeWrapper::new_with_handle(&handle, backend) {
            Ok(w) => w,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match wrapper.ps(&handle).await {
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
/// FFI: js_container_compose_logs(handle_id: i64, opts_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_logs(
    handle_id: i64,
    opts_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();

    let handle = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    let opts_json = unsafe { string_from_header(opts_ptr) };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let (service, tail) = if let Some(json) = opts_json {
            let v: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
            let svc = v.get("service").and_then(|s| s.as_str().map(|ss| ss.to_string()));
            let t = v.get("tail").and_then(|tt| tt.as_u64().map(|ttt| ttt as u32));
            (svc, t)
        } else {
            (None, None)
        };

        let backend = get_global_backend_instance_async()
            .await
            .map_err(|e| e.to_string())?;
        let wrapper = match compose::ComposeWrapper::new_with_handle(&handle, backend) {
            Ok(w) => w,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match wrapper.logs(&handle, service.as_deref(), tail).await {
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
/// FFI: js_container_compose_exec(handle_id: i64, service: *const StringHeader, cmd_json: *const StringHeader, opts_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_exec(
    handle_id: i64,
    service_ptr: *const StringHeader,
    cmd_json_ptr: *const StringHeader,
    _opts_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();

    let handle = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

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

        let backend = get_global_backend_instance_async()
            .await
            .map_err(|e| e.to_string())?;
        let wrapper = match compose::ComposeWrapper::new_with_handle(&handle, backend) {
            Ok(w) => w,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match wrapper.exec(&handle, &service, &cmd).await {
            Ok(logs) => {
                let h = types::register_container_logs(logs);
                Ok(h as u64)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Get resolved YAML config for compose stack
/// FFI: js_container_compose_config(handle_id: i64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_config(handle_id: i64) -> *mut Promise {
    let promise = js_promise_new();

    let handle = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise_deferred(
        promise as *mut u8,
        async move {
            let backend = get_global_backend_instance_async()
                .await
                .map_err(|e| e.to_string())?;
            // We need the engine from the registry to get the resolved config
            let wrapper = match compose::ComposeWrapper::new_with_handle(&handle, backend) {
                Ok(w) => w,
                Err(e) => return Err(e.to_string()),
            };
            wrapper.config().map_err(|e| e.to_string())
        },
        |yaml| unsafe {
            let str_ptr = string_to_js(&yaml);
            perry_runtime::JSValue::string_ptr(str_ptr).bits()
        },
    );

    promise
}

/// Start services in compose stack
/// FFI: js_container_compose_start(handle_id: i64, services_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_start(
    handle_id: i64,
    services_json_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();

    let handle = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    let services_json = string_from_header(services_json_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let services: Vec<String> = services_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let backend = get_global_backend_instance_async()
            .await
            .map_err(|e| e.to_string())?;
        let wrapper = match compose::ComposeWrapper::new_with_handle(&handle, backend) {
            Ok(w) => w,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match wrapper.start(&handle, &services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Stop services in compose stack
/// FFI: js_container_compose_stop(handle_id: i64, services_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_stop(
    handle_id: i64,
    services_json_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();

    let handle = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    let services_json = string_from_header(services_json_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let services: Vec<String> = services_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let backend = get_global_backend_instance_async()
            .await
            .map_err(|e| e.to_string())?;
        let wrapper = match compose::ComposeWrapper::new_with_handle(&handle, backend) {
            Ok(w) => w,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match wrapper.stop(&handle, &services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Restart services in compose stack
/// FFI: js_container_compose_restart(handle_id: i64, services_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_restart(
    handle_id: i64,
    services_json_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();

    let handle = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    let services_json = string_from_header(services_json_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let services: Vec<String> = services_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let backend = get_global_backend_instance_async()
            .await
            .map_err(|e| e.to_string())?;
        let wrapper = match compose::ComposeWrapper::new_with_handle(&handle, backend) {
            Ok(w) => w,
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match wrapper.restart(&handle, &services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

// ============ Image Operations ============

/// Inspect an image
/// FFI: js_container_inspectImage(reference: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_inspectImage(
    reference_ptr: *const StringHeader,
) -> *mut Promise {
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
        let backend = get_global_backend_instance_async()
            .await
            .map_err(|e| e.to_string())?;

        match backend.inspect_image(&reference).await {
            Ok(info) => {
                let handle_id = types::register_image_info(info);
                Ok(handle_id as u64)
            }
            Err(e) => Err(e.to_string()),
        }
    });

    promise
}

// ============ Module Initialization ============

/// Initialize the container module (called during runtime startup)
#[no_mangle]
pub extern "C" fn js_container_module_init() {
    // Proactive backend detection
    crate::common::spawn(async {
        let _ = get_global_backend_instance_async().await;
    });
}

// ============ Workload Graph FFI ============

#[no_mangle]
pub unsafe extern "C" fn js_workload_runGraph(graph_json_ptr: *const StringHeader, opts_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let graph_json = string_from_header(graph_json_ptr);
    let opts_json = string_from_header(opts_json_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let graph: workload::WorkloadGraph = graph_json.and_then(|s| serde_json::from_str(&s).ok()).ok_or("Invalid graph spec")?;
        let opts: workload::RunGraphOptions = opts_json.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(workload::RunGraphOptions { strategy: None, on_failure: None });

        let backend = get_global_backend_instance_async().await.map_err(|e| e.to_string())?;
        let engine = perry_container_compose::compose::WorkloadGraphEngine::new(backend);

        match engine.run(graph, opts).await {
            Ok(handle) => {
                // Register GraphHandle (WorkloadGraph uses u64 handle similar to Compose)
                Ok(handle.graph_id)
            }
            Err(e) => Err(e.to_string())
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_inspectGraph(graph_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let graph_json = string_from_header(graph_json_ptr);

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let graph: workload::WorkloadGraph = graph_json.and_then(|s| serde_json::from_str(&s).ok()).ok_or("Invalid graph spec")?;
        // inspectGraph returns status without starting
        Ok(workload::GraphStatus {
            nodes: graph.nodes.keys().map(|id| (id.clone(), workload::NodeState::Pending)).collect(),
            healthy: false,
            errors: HashMap::new()
        })
    }, |status| unsafe {
        let json = serde_json::to_string(&status).unwrap_or_default();
        let str_ptr = string_to_js(&json);
        perry_runtime::JSValue::string_ptr(str_ptr).bits()
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_down(handle_id: i64, opts_json_ptr: *const StringHeader) -> *mut Promise {
    // Reuse js_container_compose_down logic
    js_container_compose_down(handle_id, opts_json_ptr)
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_status(handle_id: i64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let _backend = get_global_backend_instance_async().await.map_err(|e| e.to_string())?;
        if let Some(engine) = perry_container_compose::compose::get_engine(handle_id as u64) {
             let infos = engine.ps().await.map_err(|e| e.to_string())?;
             let mut nodes = HashMap::new();
             for info in infos {
                 nodes.insert(info.name.clone(), workload::NodeState::Running);
             }
             Ok(workload::GraphStatus { nodes, healthy: true, errors: HashMap::new() })
        } else {
            Err("Graph handle not found".to_string())
        }
    }, |status| unsafe {
        let json = serde_json::to_string(&status).unwrap_or_default();
        let str_ptr = string_to_js(&json);
        perry_runtime::JSValue::string_ptr(str_ptr).bits()
    });
    promise
}
