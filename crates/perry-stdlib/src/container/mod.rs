//! Standard Library for Perry - Container Module FFI Bridge

pub mod backend;
pub mod capability;
pub mod compose;
pub(crate) mod mod_private;
pub mod types;
pub mod verification;


use perry_runtime::{js_promise_new, Promise, StringHeader};
use perry_container_compose::types::ComposeServiceBuild;

pub use backend::{detect_backend, ContainerBackend};
pub use types::{
    ComposeHandle, ComposeSpec, ContainerError, ContainerHandle,
    ContainerInfo, ContainerLogs, ContainerSpec, ImageInfo, ListOrDict,
};

use self::mod_private::{get_cached_backend_name, get_global_backend_instance};

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
        let backend = get_global_backend_instance().await
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
        let backend = get_global_backend_instance().await
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
        let backend = get_global_backend_instance().await
            .map_err(|e| e.to_string())?;
        backend.start(&id).await
            .map(|_| 0u64)
            .map_err(|e| e.to_string())
    });

    promise
}

/// Stop a running container
/// FFI: js_container_stop(id: *const StringHeader, options_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_stop(
    id_ptr: *const StringHeader,
    options_ptr: *const StringHeader,
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

    let options_json = string_from_header(options_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let timeout_opt = options_json
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v.get("timeout").and_then(|t| t.as_u64()))
            .map(|t| t as u32);

        let backend = get_global_backend_instance().await
            .map_err(|e| e.to_string())?;
        backend.stop(&id, timeout_opt).await
            .map(|_| 0u64)
            .map_err(|e| e.to_string())
    });

    promise
}

/// Remove a container
/// FFI: js_container_remove(id: *const StringHeader, options_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_remove(
    id_ptr: *const StringHeader,
    options_ptr: *const StringHeader,
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

    let options_json = string_from_header(options_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let force = options_json
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v.get("force").and_then(|f| f.as_bool()))
            .unwrap_or(false);

        let backend = get_global_backend_instance().await
            .map_err(|e| e.to_string())?;
        backend.remove(&id, force).await
            .map(|_| 0u64)
            .map_err(|e| e.to_string())
    });

    promise
}

/// List containers
/// FFI: js_container_list(options_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_list(options_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let options_json = string_from_header(options_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let all = options_json
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v.get("all").and_then(|a| a.as_bool()))
            .unwrap_or(false);

        let backend = get_global_backend_instance().await
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
        let backend = get_global_backend_instance().await
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
    string_to_js(get_cached_backend_name())
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
                Ok(json)
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
/// FFI: js_container_logs(id: *const StringHeader, options_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_logs(
    id_ptr: *const StringHeader,
    options_ptr: *const StringHeader,
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

    let options_json = string_from_header(options_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let tail_opt = options_json
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v.get("tail").and_then(|t| t.as_u64()))
            .map(|t| t as u32);

        let backend = get_global_backend_instance().await
            .map_err(|e| e.to_string())?;
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
/// FFI: js_container_exec(id: *const StringHeader, cmd_json: *const StringHeader, env_json: *const StringHeader, workdir_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_exec(
    id_ptr: *const StringHeader,
    cmd_json_ptr: *const StringHeader,
    env_json_ptr: *const StringHeader,
    workdir_json_ptr: *const StringHeader,
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
    let workdir_json = string_from_header(workdir_json_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let cmd: Vec<String> = cmd_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let env: Option<std::collections::HashMap<String, String>> = env_json
            .and_then(|s| serde_json::from_str(&s).ok());

        let workdir = workdir_json
            .and_then(|s| serde_json::from_str::<String>(&s).ok());

        let backend = get_global_backend_instance().await
            .map_err(|e| e.to_string())?;
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

/// Build a container image from a spec
/// FFI: js_container_build(spec_json: *const StringHeader, image_name: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_build(
    spec_ptr: *const StringHeader,
    name_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();

    let spec = match string_from_header(spec_ptr)
        .and_then(|s| serde_json::from_str::<ComposeServiceBuild>(&s).ok())
    {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid build spec".to_string())
            });
            return promise;
        }
    };

    let name = match string_from_header(name_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid image name".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await
            .map_err(|e| e.to_string())?;
        backend.build(&spec, &name).await
            .map(|_| 0u64)
            .map_err(|e| e.to_string())
    });

    promise
}

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
        let backend = get_global_backend_instance().await
            .map_err(|e| e.to_string())?;
        backend.pull_image(&reference).await
            .map(|_| 0u64)
            .map_err(|e| e.to_string())
    });

    promise
}

/// List images
/// FFI: js_container_listImages() -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_listImages() -> *mut Promise {
    let promise = js_promise_new();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await
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
        let backend = get_global_backend_instance().await
            .map_err(|e| e.to_string())?;
        backend.remove_image(&reference, force != 0).await
            .map(|_| 0u64)
            .map_err(|e| e.to_string())
    });

    promise
}

// ============ Compose Functions ============

/// Bring up a Compose stack
/// FFI: js_container_composeUp(spec_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_composeUp(spec_ptr: *const perry_runtime::StringHeader) -> *mut Promise {
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
        compose::compose_up(spec).await
            .map(|h| h.stack_id)
            .map_err(|e| e.to_string())
    }, |stack_id| {
        perry_runtime::JSValue::number(stack_id as f64).bits()
    });

    promise
}

/// Stop and remove compose stack.
/// FFI: js_container_compose_down(handle_id: i64, volumes: i32) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_down(handle_id: i64, volumes: i32) -> *mut Promise {
    let promise = js_promise_new();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        compose::compose_down(handle_id as u64, volumes != 0).await
            .map(|_| 0u64)
            .map_err(|e| e.to_string())
    });

    promise
}

/// Get container info for compose stack
/// FFI: js_container_compose_ps(handle_id: i64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_ps(handle_id: i64) -> *mut Promise {
    let promise = js_promise_new();

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        compose::compose_ps(handle_id as u64).await
            .map_err(|e| e.to_string())
    }, |containers| {
        let h = types::register_container_info_list(containers);
        perry_runtime::JSValue::number(h as f64).bits()
    });

    promise
}

/// Get logs from compose stack
/// FFI: js_container_compose_logs(handle_id: i64, options_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_logs(
    handle_id: i64,
    options_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();

    let options_json = string_from_header(options_ptr);

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let options: serde_json::Value = options_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

        let service = options.get("service").and_then(|v| v.as_str());
        let tail = options.get("tail").and_then(|v| v.as_u64()).map(|v| v as u32);

        compose::compose_logs(handle_id as u64, service, tail).await
            .map_err(|e| e.to_string())
    }, |logs| {
        let h = types::register_container_logs(logs);
        perry_runtime::JSValue::number(h as f64).bits()
    });

    promise
}

/// Execute command in compose service
/// FFI: js_container_compose_exec(handle_id: i64, service: *const StringHeader, cmd_json: *const StringHeader, options_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_exec(
    handle_id: i64,
    service_ptr: *const StringHeader,
    cmd_json_ptr: *const StringHeader,
    _options_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();

    let service_opt = string_from_header(service_ptr);
    let cmd_json = string_from_header(cmd_json_ptr);

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let service = match service_opt {
            Some(s) => s,
            None => return Err("Invalid service name".to_string()),
        };

        let cmd: Vec<String> = cmd_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        compose::compose_exec(handle_id as u64, service, cmd).await
            .map_err(|e| e.to_string())
    }, |logs| {
        let h = types::register_container_logs(logs);
        perry_runtime::JSValue::number(h as f64).bits()
    });

    promise
}

/// Get resolved configuration for compose stack
/// FFI: js_container_compose_config(handle_id: i64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_config(handle_id: i64) -> *mut Promise {
    let promise = js_promise_new();

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        compose::compose_config(handle_id as u64).await
            .map_err(|e| e.to_string())
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

    let services_json = string_from_header(services_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let services: Vec<String> = services_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        compose::compose_start(handle_id as u64, services).await
            .map(|_| 0u64)
            .map_err(|e| e.to_string())
    });

    promise
}

/// Stop services in compose stack
/// FFI: js_container_compose_stop(handle_id: i64, services_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_stop(handle_id: i64, services_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let services_json = string_from_header(services_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let services: Vec<String> = services_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        compose::compose_stop(handle_id as u64, services).await
            .map(|_| 0u64)
            .map_err(|e| e.to_string())
    });

    promise
}

/// Restart services in compose stack
/// FFI: js_container_compose_restart(handle_id: i64, services_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_restart(handle_id: i64, services_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let services_json = string_from_header(services_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let services: Vec<String> = services_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        compose::compose_restart(handle_id as u64, services).await
            .map(|_| 0u64)
            .map_err(|e| e.to_string())
    });

    promise
}

// ============ Module Initialization ============

/// Initialize the container module (called during runtime startup)
#[no_mangle]
pub extern "C" fn js_container_module_init() {
}
