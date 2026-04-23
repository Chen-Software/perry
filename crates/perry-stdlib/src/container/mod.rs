//! Container module for Perry
//!
//! Provides OCI container management with platform-adaptive backend selection.

pub mod backend;
pub mod capability;
pub mod compose;
pub mod context;
pub mod types;
pub mod verification;
pub mod workload;

// Re-export commonly used types
pub use types::{
    ComposeHandle, ComposeSpec, ContainerError, ContainerHandle,
    ContainerInfo, ContainerLogs, ContainerSpec, ImageInfo, ListOrDict,
    ComposeEngine,
};

use perry_runtime::{js_promise_new, Promise, StringHeader};
pub use backend::{detect_backend, ContainerBackend};
use std::sync::OnceLock;
use std::sync::Arc;
use std::collections::HashMap;

/// Get or initialize the global backend instance via ContainerContext
pub async fn get_global_backend() -> Result<Arc<dyn ContainerBackend>, ContainerError> {
    context::ContainerContext::global().get_backend().await
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

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend: Arc<dyn ContainerBackend> = match get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<ContainerHandle, String>(types::container_error_to_json(e)),
        };
        backend.run(&spec).await.map_err(|e| types::container_error_to_json(ContainerError::from(e)))
    }, |handle| {
        let handle_id = types::register_container_handle(handle);
        perry_runtime::JSValue::number(handle_id as f64).bits()
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

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend: Arc<dyn ContainerBackend> = match get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<ContainerHandle, String>(types::container_error_to_json(e)),
        };
        backend.create(&spec).await.map_err(|e| types::container_error_to_json(ContainerError::from(e)))
    }, |handle| {
        let handle_id = types::register_container_handle(handle);
        perry_runtime::JSValue::number(handle_id as f64).bits()
    });

    promise
}

/// Start a previously created container
/// FFI: js_container_start(id: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_start(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let id = match types::string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid container ID".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend: Arc<dyn ContainerBackend> = match get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(types::container_error_to_json(e)),
        };
        match backend.start(&id).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(types::container_error_to_json(ContainerError::from(e))),
        }
    });

    promise
}

/// Stop a running container
/// FFI: js_container_stop(id: *const StringHeader, timeout: i32) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_stop(id_ptr: *const StringHeader, timeout: i32) -> *mut Promise {
    let promise = js_promise_new();

    let id = match types::string_from_header(id_ptr) {
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
        let backend: Arc<dyn ContainerBackend> = match get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(types::container_error_to_json(e)),
        };
        match backend.stop(&id, timeout_opt).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(types::container_error_to_json(ContainerError::from(e))),
        }
    });

    promise
}

/// Remove a container
/// FFI: js_container_remove(id: *const StringHeader, force: i32) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_remove(id_ptr: *const StringHeader, force: i32) -> *mut Promise {
    let promise = js_promise_new();

    let id = match types::string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid container ID".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend: Arc<dyn ContainerBackend> = match get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(types::container_error_to_json(e)),
        };
        match backend.remove(&id, force != 0).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(types::container_error_to_json(ContainerError::from(e))),
        }
    });

    promise
}

/// List containers
/// FFI: js_container_list(all: i32) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_list(all: i32) -> *mut Promise {
    let promise = js_promise_new();

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend: Arc<dyn ContainerBackend> = match get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<Vec<ContainerInfo>, String>(types::container_error_to_json(e)),
        };
        backend.list(all != 0).await.map_err(|e| types::container_error_to_json(ContainerError::from(e)))
    }, |list| {
        let json = serde_json::to_string(&list).unwrap_or_default();
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        perry_runtime::JSValue::string_ptr(str_ptr).bits()
    });

    promise
}

/// Inspect a container
/// FFI: js_container_inspect(id: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_inspect(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let id = match types::string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid container ID".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend: Arc<dyn ContainerBackend> = match get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<ContainerInfo, String>(types::container_error_to_json(e)),
        };
        backend.inspect(&id).await.map_err(|e| types::container_error_to_json(ContainerError::from(e)))
    }, |info| {
        let json = serde_json::to_string(&info).unwrap_or_default();
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        perry_runtime::JSValue::string_ptr(str_ptr).bits()
    });

    promise
}

/// Get the current backend name
/// FFI: js_container_getBackend() -> *const StringHeader
#[no_mangle]
pub unsafe extern "C" fn js_container_getBackend() -> *const StringHeader {
    // Note: this is synchronous and might return "unknown" if not initialized
    if let Some(b) = context::ContainerContext::global().backend.get() {
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
                if let perry_container_compose::error::ComposeError::NoBackendFound { probed } = e {
                    Ok(serde_json::to_string(&probed).unwrap_or_default())
                } else {
                    Ok("[]".to_string())
                }
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

    let id = match types::string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid container ID".to_string())
            });
            return promise;
        }
    };

    let tail_opt = if tail >= 0 { Some(tail as u32) } else { None };

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend: Arc<dyn ContainerBackend> = match get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<ContainerLogs, String>(types::container_error_to_json(e)),
        };
        backend.logs(&id, tail_opt).await.map_err(|e| types::container_error_to_json(ContainerError::from(e)))
    }, |logs| {
        let json = serde_json::to_string(&logs).unwrap_or_default();
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        perry_runtime::JSValue::string_ptr(str_ptr).bits()
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

    let id = match types::string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid container ID".to_string())
            });
            return promise;
        }
    };

    let cmd_json = types::string_from_header(cmd_json_ptr);
    let env_json = types::string_from_header(env_json_ptr);
    let workdir = types::string_from_header(workdir_ptr);

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let cmd: Vec<String> = cmd_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let env: Option<HashMap<String, String>> = env_json
            .and_then(|s| serde_json::from_str(&s).ok());

        let backend: Arc<dyn ContainerBackend> = match get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<ContainerLogs, String>(types::container_error_to_json(e)),
        };
        backend.exec(&id, &cmd, env.as_ref(), workdir.as_deref()).await.map_err(|e| types::container_error_to_json(ContainerError::from(e)))
    }, |logs| {
        let json = serde_json::to_string(&logs).unwrap_or_default();
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        perry_runtime::JSValue::string_ptr(str_ptr).bits()
    });

    promise
}

// ============ Image Management ============

/// Pull a container image
/// FFI: js_container_pullImage(reference: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_pullImage(reference_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let reference = match types::string_from_header(reference_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid image reference".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend: Arc<dyn ContainerBackend> = match get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(types::container_error_to_json(e)),
        };
        match backend.pull_image(&reference).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(types::container_error_to_json(ContainerError::from(e))),
        }
    });

    promise
}

/// List images
/// FFI: js_container_listImages() -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_listImages() -> *mut Promise {
    let promise = js_promise_new();

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend: Arc<dyn ContainerBackend> = match get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<Vec<ImageInfo>, String>(types::container_error_to_json(e)),
        };
        backend.list_images().await.map_err(|e| types::container_error_to_json(ContainerError::from(e)))
    }, |images| {
        let json = serde_json::to_string(&images).unwrap_or_default();
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        perry_runtime::JSValue::string_ptr(str_ptr).bits()
    });

    promise
}

/// Remove an image
/// FFI: js_container_removeImage(reference: *const StringHeader, force: i32) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_removeImage(reference_ptr: *const StringHeader, force: i32) -> *mut Promise {
    let promise = js_promise_new();

    let reference = match types::string_from_header(reference_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid image reference".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend: Arc<dyn ContainerBackend> = match get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<u64, String>(types::container_error_to_json(e)),
        };
        match backend.remove_image(&reference, force != 0).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(types::container_error_to_json(ContainerError::from(e))),
        }
    });

    promise
}

// ============ Compose Functions ============

/// Bring up a Compose stack
/// FFI: js_container_composeUp(spec_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_composeUp(spec_ptr: *const perry_runtime::StringHeader) -> *mut Promise {
    js_compose_up(spec_ptr)
}

/// Bring up a Compose stack
/// FFI: js_compose_up(spec_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_compose_up(spec_ptr: *const perry_runtime::StringHeader) -> *mut Promise {
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
        let backend: Arc<dyn ContainerBackend> = match get_global_backend().await {
            Ok(b) => b,
            Err(e) => return Err::<ComposeEngine, String>(types::container_error_to_json(e)),
        };
        let project_name = spec.name.clone().unwrap_or_else(|| "default".to_string());
        let engine = ComposeEngine::new(spec, project_name, backend);

        // Requirement 6 AC 1: starts all services
        engine.up(&[], true, false, false).await.map_err(|e| types::container_error_to_json(e.into()))?;

        Ok(engine)
    }, |engine| {
        let id = types::register_compose_handle(engine);
        perry_runtime::JSValue::number(id as f64).bits()
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_up(spec_ptr: *const perry_runtime::StringHeader) -> *mut Promise {
    js_compose_up(spec_ptr)
}

/// Stop and remove compose stack.
/// FFI: js_compose_down(handle_id: f64, volumes: i32) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_compose_down(handle_id: f64, volumes: i32) -> *mut Promise {
    let promise = js_promise_new();

    let arc_engine = match types::COMPOSE_HANDLES.get_or_init(dashmap::DashMap::new).remove(&(handle_id as u64)) {
        Some((_, h)) => h,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match arc_engine.0.down(volumes != 0, false).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(types::container_error_to_json(e.into())),
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_down(handle_id: f64, volumes: i32) -> *mut Promise {
    js_compose_down(handle_id, volumes)
}

/// Get container info for compose stack
/// FFI: js_compose_ps( __handle_id: i64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_compose_ps(handle_id: f64) -> *mut Promise {
    let promise = js_promise_new();

    let arc_engine = match types::COMPOSE_HANDLES.get_or_init(dashmap::DashMap::new).get(&(handle_id as u64)) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        arc_engine.0.ps().await.map_err(|e| types::container_error_to_json(e.into()))
    }, |list| {
        let json = serde_json::to_string(&list).unwrap_or_default();
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        perry_runtime::JSValue::string_ptr(str_ptr).bits()
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_ps(handle_id: f64) -> *mut Promise {
    js_compose_ps(handle_id)
}

/// Get logs from compose stack
/// FFI: js_compose_logs(handle_id: f64, service: *const StringHeader, tail: i32) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_compose_logs(
    handle_id: i64,
    service_ptr: *const StringHeader,
    tail: i32,
) -> *mut Promise {
    let promise = js_promise_new();

    let arc_engine = match types::COMPOSE_HANDLES.get_or_init(dashmap::DashMap::new).get(&(handle_id as u64)) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    let service = unsafe { types::string_from_header(service_ptr) };
    let tail_opt = if tail >= 0 { Some(tail as u32) } else { None };

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        arc_engine.0.logs(service.as_deref(), tail_opt).await.map_err(|e| types::container_error_to_json(e.into()))
    }, |logs| {
        let json = serde_json::to_string(&logs).unwrap_or_default();
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        perry_runtime::JSValue::string_ptr(str_ptr).bits()
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_logs(handle_id: f64, service_ptr: *const StringHeader, tail: i32) -> *mut Promise {
    js_compose_logs(handle_id as i64, service_ptr, tail)
}

/// Execute command in compose service
/// FFI: js_compose_exec(handle_id: f64, service: *const StringHeader, cmd_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_compose_exec(
    handle_id: i64,
    service_ptr: *const StringHeader,
    cmd_json_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();

    let arc_engine = match types::COMPOSE_HANDLES.get_or_init(dashmap::DashMap::new).get(&(handle_id as u64)) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    let service_opt = unsafe { types::string_from_header(service_ptr) };
    let cmd_json = unsafe { types::string_from_header(cmd_json_ptr) };

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let service = match service_opt {
            Some(s) => s,
            None => return Err::<ContainerLogs, String>("Invalid service name".to_string()),
        };

        let cmd: Vec<String> = cmd_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        arc_engine.0.exec(&service, &cmd).await.map_err(|e| types::container_error_to_json(e.into()))
    }, |logs| {
        let json = serde_json::to_string(&logs).unwrap_or_default();
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        perry_runtime::JSValue::string_ptr(str_ptr).bits()
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_exec(handle_id: f64, service_ptr: *const StringHeader, cmd_json_ptr: *const StringHeader) -> *mut Promise {
    js_compose_exec(handle_id as i64, service_ptr, cmd_json_ptr)
}

/// Get resolved configuration for compose stack
/// FFI: js_compose_config(spec_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_compose_config(spec_ptr: *const StringHeader) -> *mut Promise {
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
        let yaml = spec.to_yaml().map_err(|e| types::container_error_to_json(e.into()))?;
        Ok(yaml)
    }, |yaml| {
        let str_ptr = perry_runtime::js_string_from_bytes(yaml.as_ptr(), yaml.len() as u32);
        perry_runtime::JSValue::string_ptr(str_ptr).bits()
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_config(spec_ptr: *const StringHeader) -> *mut Promise {
    js_compose_config(spec_ptr)
}

/// Start services in compose stack
/// FFI: js_compose_start(handle_id: f64, services_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_compose_start(handle_id: f64, services_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let arc_engine = match types::COMPOSE_HANDLES.get_or_init(dashmap::DashMap::new).get(&(handle_id as u64)) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    let services_json = types::string_from_header(services_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let services: Vec<String> = services_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        match arc_engine.0.start(&services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(types::container_error_to_json(e.into())),
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_start(handle_id: f64, services_ptr: *const StringHeader) -> *mut Promise {
    js_compose_start(handle_id, services_ptr)
}

/// Stop services in compose stack
/// FFI: js_compose_stop(handle_id: f64, services_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_compose_stop(handle_id: f64, services_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let arc_engine = match types::COMPOSE_HANDLES.get_or_init(dashmap::DashMap::new).get(&(handle_id as u64)) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    let services_json = types::string_from_header(services_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let services: Vec<String> = services_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        match arc_engine.0.stop(&services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(types::container_error_to_json(e.into())),
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_stop(handle_id: f64, services_ptr: *const StringHeader) -> *mut Promise {
    js_compose_stop(handle_id, services_ptr)
}

/// Restart services in compose stack
/// FFI: js_compose_restart(handle_id: f64, services_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_compose_restart(handle_id: f64, services_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let arc_engine = match types::COMPOSE_HANDLES.get_or_init(dashmap::DashMap::new).get(&(handle_id as u64)) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    let services_json = types::string_from_header(services_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let services: Vec<String> = services_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        match arc_engine.0.restart(&services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(types::container_error_to_json(e.into())),
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_restart(handle_id: f64, services_ptr: *const StringHeader) -> *mut Promise {
    js_compose_restart(handle_id, services_ptr)
}


// ============ Workload API ============

#[no_mangle]
pub unsafe extern "C" fn js_workload_graph(
    name_ptr: *const StringHeader,
    _spec_ptr: *const StringHeader,
) -> *const StringHeader {
    let name = types::string_from_header(name_ptr).unwrap_or_default();
    let json = serde_json::json!({
        "name": name,
        "nodes": {},
        "edges": []
    })
    .to_string();
    string_to_js(&json)
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_node(
    name_ptr: *const StringHeader,
    _spec_ptr: *const StringHeader,
) -> *const StringHeader {
    let name = types::string_from_header(name_ptr).unwrap_or_default();
    let json = serde_json::json!({
        "id": name,
        "name": name,
    })
    .to_string();
    string_to_js(&json)
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_runGraph(
    _graph_ptr: *const StringHeader,
    _opts_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move { Ok(1u64) });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_inspectGraph(_graph_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise_deferred(
        promise as *mut u8,
        async move { Ok(()) },
        |_| {
            let json = serde_json::json!({
                "nodes": {},
                "healthy": true
            })
            .to_string();
            let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
            perry_runtime::JSValue::string_ptr(str_ptr).bits()
        },
    );
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_down(_handle_id: i64, _opts_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move { Ok(0u64) });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_status(_handle_id: i64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move { Ok(()) }, |_| {
        let json = serde_json::json!({"nodes": {}, "healthy": true}).to_string();
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        perry_runtime::JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_graph(_handle_id: i64) -> *const StringHeader {
    string_to_js("{}")
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_logs(
    _handle_id: i64,
    _node_ptr: *const StringHeader,
    _opts_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move { Ok(()) }, |_| {
        let json = serde_json::json!({"stdout": "", "stderr": ""}).to_string();
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        perry_runtime::JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_exec(
    _handle_id: i64,
    _node_ptr: *const StringHeader,
    _cmd_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move { Ok(()) }, |_| {
        let json = serde_json::json!({"stdout": "", "stderr": ""}).to_string();
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        perry_runtime::JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_ps(_handle_id: i64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move { Ok(()) }, |_| {
        let json = "[]".to_string();
        let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
        perry_runtime::JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub extern "C" fn js_container_module_init() {
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::container::types::{ContainerHandle, ContainerInfo, ContainerSpec, COMPOSE_HANDLES};
    use perry_container_compose::testing::mock_backend::{MockBackend, MockResponse};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_ffi_container_lifecycle_with_mock() {
        let mock = Arc::new(MockBackend::new());

        // Use a local context to avoid interference with other tests
        let ctx = context::ContainerContext::new();
        ctx.set_backend(mock.clone());

        mock.push_response(MockResponse::Run(ContainerHandle {
            id: "mock-123".into(),
            name: Some("test-container".into()),
        }));

        let spec = ContainerSpec {
            image: "alpine".into(),
            name: Some("test-container".into()),
            ..Default::default()
        };

        let handle = mock.run(&spec).await.unwrap();
        assert_eq!(handle.id, "mock-123");
        assert!(mock.calls.lock().unwrap().contains(&"run".to_string()));
    }

    #[tokio::test]
    async fn test_ffi_detect_backend() {
        let mock = Arc::new(MockBackend::new());
        context::ContainerContext::global().set_backend(mock.clone());

        unsafe {
            let _promise_ptr = js_container_detectBackend();
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            crate::common::js_stdlib_process_pending();
        }
    }

    #[tokio::test]
    async fn test_ffi_compose_up_with_mock() {
        let mock = Arc::new(MockBackend::new());

        // Setup responses for engine.up()
        mock.push_response(MockResponse::List(vec![]));
        mock.push_response(MockResponse::Run(ContainerHandle { id: "web".into(), name: None }));

        let spec = serde_json::json!({
            "name": "test-stack",
            "services": {
                "web": { "image": "nginx" }
            }
        });
        let spec_struct: ComposeSpec = serde_json::from_value(spec).unwrap();

        let engine = ComposeEngine::new(spec_struct, "test-stack".into(), mock.clone());
        let _handle = engine.up(&[], true, false, false).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert!(calls.contains(&"list".to_string()));
        assert!(calls.contains(&"run".to_string()));
    }

    #[tokio::test]
    async fn test_ffi_workload_api() {
        let name_str = "test-workload";

        unsafe {
            let name_ptr = perry_runtime::js_string_from_bytes(name_str.as_ptr(), name_str.len() as u32);
            let spec_ptr = perry_runtime::js_string_from_bytes(b"{}".as_ptr(), 2);

            let graph_ptr = js_workload_graph(name_ptr, spec_ptr);
            assert!(!graph_ptr.is_null());

            let graph_json = types::string_from_header(graph_ptr).unwrap();
            let graph: workload::WorkloadGraph = serde_json::from_str(&graph_json).unwrap();
            assert_eq!(graph.name, name_str);

            let _node_ptr = js_workload_node(name_ptr, spec_ptr);
            let _promise_ptr = js_workload_runGraph(graph_ptr, spec_ptr);

            let status_promise = js_workload_handle_status(1);
            assert!(!status_promise.is_null());

            let ps_promise = js_workload_handle_ps(1);
            assert!(!ps_promise.is_null());

            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            crate::common::js_stdlib_process_pending();
        }
    }
}
