//! Container module for Perry
//!
//! Provides OCI container management with platform-adaptive backend selection.
//! Uses apple/container on macOS/iOS and podman on all other platforms.

pub mod backend;
pub mod capability;
pub mod compose;
pub mod types;
pub mod verification;

// Re-export commonly used types
pub use types::{
    ComposeNetwork, ComposeService, ComposeSpec, ComposeVolume, ContainerError, ContainerHandle,
    ContainerInfo, ContainerLogs, ContainerSpec, ImageInfo, ListOrDict,
};

use backend::ContainerBackend;
use perry_runtime::{js_promise_new, js_string_from_bytes, JSValue, Promise, StringHeader};
use std::sync::Arc;
use std::sync::OnceLock;

static BACKEND: OnceLock<Arc<dyn ContainerBackend>> = OnceLock::new();

fn get_global_backend_instance() -> Result<&'static Arc<dyn ContainerBackend>, types::ContainerError>
{
    if let Some(b) = BACKEND.get() {
        return Ok(b);
    }
    let b = perry_container_compose::backend::get_backend()
        .map(Arc::from)
        .map_err(|e| types::ContainerError::BackendError {
            code: 1,
            message: e.to_string(),
        })?;
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

    let spec: types::ContainerSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(format!("Invalid ContainerSpec: {}", msg))
            });
            return promise;
        }
    };

    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
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

    let spec: types::ContainerSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(format!("Invalid ContainerSpec: {}", msg))
            });
            return promise;
        }
    };

    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
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

    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
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

#[no_mangle]
pub unsafe extern "C" fn js_container_stop(
    id_ptr: *const StringHeader,
    timeout: i32,
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

    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let timeout_opt = if timeout < 0 {
            None
        } else {
            Some(timeout as u32)
        };
        match backend.stop(&id, timeout_opt).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

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

    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
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

#[no_mangle]
pub unsafe extern "C" fn js_container_list(all: i32) -> *mut Promise {
    let promise = js_promise_new();

    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
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

    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
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

#[no_mangle]
pub unsafe extern "C" fn js_container_getBackend() -> *const StringHeader {
    let backend_name = match get_global_backend_instance() {
        Ok(b) => b.backend_name(),
        Err(_) => "unknown",
    };
    string_to_js(backend_name)
}

// ============ Container Logs and Exec ============

#[no_mangle]
pub unsafe extern "C" fn js_container_logs(
    id_ptr: *const StringHeader,
    _follow: i32,
    tail: i32,
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

    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let tail_opt = if tail < 0 { None } else { Some(tail as u32) };
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

    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let cmd: Vec<String> = cmd_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let env: Option<std::collections::HashMap<String, String>> =
            env_json.and_then(|s| serde_json::from_str(&s).ok());

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

    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
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

#[no_mangle]
pub unsafe extern "C" fn js_container_listImages() -> *mut Promise {
    let promise = js_promise_new();

    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
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

    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
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

#[no_mangle]
pub unsafe extern "C" fn js_compose_up(spec_json_ptr: *const StringHeader) -> *mut Promise {
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

    let spec: types::ComposeSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(format!("Invalid ComposeSpec: {}", msg))
            });
            return promise;
        }
    };

    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
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

#[no_mangle]
pub unsafe extern "C" fn js_compose_down(handle_id: i64, volumes: i32) -> *mut Promise {
    let promise = js_promise_new();

    let handle = match types::take_compose_handle(handle_id as u64) {
        Some(h) => h,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let wrapper = compose::ComposeWrapper::new(
            types::ComposeSpec::default(),
            backend,
        );
        match wrapper.down(&handle, volumes != 0).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_ps(handle_id: i64) -> *mut Promise {
    let promise = js_promise_new();

    let handle = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };
    let handle = handle.clone();

    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let wrapper = compose::ComposeWrapper::new(
            types::ComposeSpec::default(),
            backend,
        );
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

#[no_mangle]
pub unsafe extern "C" fn js_compose_logs(
    handle_id: i64,
    service_ptr: *const StringHeader,
    tail: i32,
) -> *mut Promise {
    let promise = js_promise_new();

    let handle = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };
    let handle = handle.clone();

    let service = string_from_header(service_ptr);
    let tail_opt = if tail >= 0 { Some(tail as u32) } else { None };

    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let wrapper = compose::ComposeWrapper::new(
            types::ComposeSpec::default(),
            backend,
        );
        match wrapper.logs(&handle, service.as_deref(), tail_opt).await {
            Ok(logs) => {
                let h = types::register_container_logs(logs);
                Ok(h as u64)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_exec(
    handle_id: i64,
    service_ptr: *const StringHeader,
    cmd_json_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();

    let handle = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };
    let handle = handle.clone();

    let service = match string_from_header(service_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid service name".to_string())
            });
            return promise;
        }
    };

    let cmd_str = match string_from_header(cmd_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid command string".to_string())
            });
            return promise;
        }
    };

    let cmd: Vec<String> = cmd_str.split_whitespace().map(String::from).collect();

    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let wrapper = compose::ComposeWrapper::new(
            types::ComposeSpec::default(),
            backend,
        );
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

#[no_mangle]
pub unsafe extern "C" fn js_compose_config(spec_json_ptr: *const StringHeader) -> *mut Promise {
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

    let spec: types::ComposeSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(format!("Invalid ComposeSpec: {}", msg))
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match spec.to_yaml() {
            Ok(yaml) => {
                let h = types::register_container_handle(types::ContainerHandle {
                    id: yaml,
                    name: Some("config".to_string()),
                });
                Ok(h)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_start(handle_id: i64, services_json: *const StringHeader) -> *mut Promise {
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

    let services: Vec<String> = string_from_header(services_json)
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let wrapper = compose::ComposeWrapper::new(types::ComposeSpec::default(), backend);
        match wrapper.start(&handle, &services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_stop(handle_id: i64, services_json: *const StringHeader) -> *mut Promise {
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

    let services: Vec<String> = string_from_header(services_json)
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let wrapper = compose::ComposeWrapper::new(types::ComposeSpec::default(), backend);
        match wrapper.stop(&handle, &services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_restart(handle_id: i64, services_json: *const StringHeader) -> *mut Promise {
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

    let services: Vec<String> = string_from_header(services_json)
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let wrapper = compose::ComposeWrapper::new(types::ComposeSpec::default(), backend);
        match wrapper.restart(&handle, &services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

// ============ Image Verification ============

#[no_mangle]
pub unsafe extern "C" fn js_container_verifyImage(reference_ptr: *const StringHeader) -> *mut Promise {
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
        match verification::verify_image(&reference).await {
            Ok(digest) => {
                Ok(digest.len() as u64)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

// ============ Capability (Sandboxed Execution) ============

#[no_mangle]
pub unsafe extern "C" fn js_container_runCapability(command_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let command = match string_from_header(command_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid command".to_string())
            });
            return promise;
        }
    };

    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
            });
            return promise;
        }
    };

    let config = capability::CapabilityConfig::default();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match capability::run_capability(&backend, &command, &config).await {
            Ok(result) => {
                let logs = types::ContainerLogs {
                    stdout: result.stdout,
                    stderr: result.stderr,
                };
                let h = types::register_container_logs(logs);
                Ok(h as u64)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

// ============ Network Management ============

#[no_mangle]
pub unsafe extern "C" fn js_container_createNetwork(name_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let name = match string_from_header(name_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid network name".to_string())
            });
            return promise;
        }
    };

    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let config = types::ComposeNetwork::default();
        match backend.create_network(&name, &config).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_removeNetwork(name_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let name = match string_from_header(name_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid network name".to_string())
            });
            return promise;
        }
    };

    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match backend.remove_network(&name).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

// ============ Volume Management ============

#[no_mangle]
pub unsafe extern "C" fn js_container_createVolume(name_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let name = match string_from_header(name_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid volume name".to_string())
            });
            return promise;
        }
    };

    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let config = types::ComposeVolume::default();
        match backend.create_volume(&name, &config).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_removeVolume(name_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let name = match string_from_header(name_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid volume name".to_string())
            });
            return promise;
        }
    };

    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(msg)
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match backend.remove_volume(&name).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

// ============ Module Initialization ============

#[no_mangle]
pub extern "C" fn js_container_module_init() {
    let _ = get_global_backend_instance();
}
