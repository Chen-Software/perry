//! Container module for Perry
//!
//! Provides OCI container management with platform-adaptive backend selection.

pub mod backend;
pub mod capability;
pub mod compose;
pub mod types;
pub mod verification;

mod mod_private {
    use super::get_global_backend;
    use crate::container::backend::ContainerBackend;
    use std::sync::Arc;

    pub async fn get_global_backend_instance() -> Result<Arc<dyn ContainerBackend>, String> {
        get_global_backend()
            .await
            .map(|b| Arc::clone(b))
            .map_err(|e| e.to_string())
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
use std::sync::OnceLock;

// Global backend instance - initialized once at first use
static BACKEND: OnceLock<Arc<dyn ContainerBackend>> = OnceLock::new();
static BACKEND_INIT_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Get or initialize the global backend instance.
async fn get_global_backend() -> Result<&'static Arc<dyn ContainerBackend>, ContainerError> {
    if let Some(b) = BACKEND.get() {
        return Ok(b);
    }

    let _guard = BACKEND_INIT_MUTEX.lock().await;

    if let Some(b) = BACKEND.get() {
        return Ok(b);
    }

    let b = match detect_backend().await {
        Ok(backend) => Arc::from(backend) as Arc<dyn ContainerBackend>,
        Err(e) => {
            use std::io::IsTerminal;
            let interactive = std::io::stderr().is_terminal();
            let prompt_disabled = std::env::var("PERRY_NO_INSTALL_PROMPT").is_ok();
            if interactive && !prompt_disabled {
                let installer = perry_container_compose::BackendInstaller::new();
                match installer.run().await {
                    Ok(backend) => Arc::from(backend) as Arc<dyn ContainerBackend>,
                    Err(_) => return Err(ContainerError::from(e)),
                }
            } else {
                return Err(ContainerError::from(e));
            }
        }
    };

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

/// `POINTER_TAG` for NaN-boxing a handle id as an opaque pointer.
const POINTER_TAG_BITS: u64 = 0x7FFD_0000_0000_0000;

/// Encode a u64 handle id as the f64 bits a Promise resolution slot expects.
#[inline]
fn handle_to_promise_bits(id: u64) -> u64 {
    POINTER_TAG_BITS | (id & 0x0000_FFFF_FFFF_FFFF)
}

/// `TAG_UNDEFINED` as raw f64 bits.
const PROMISE_VOID_BITS: u64 = 0x7FFC_0000_0000_0001;

/// Decode a NaN-boxed f64 receiver/handle back to its registry id (i64).
#[inline]
fn handle_id_from_f64(boxed: f64) -> i64 {
    (boxed.to_bits() & 0x0000_FFFF_FFFF_FFFF) as i64
}

/// Optionally verify a container image's signature before pulling/running.
async fn maybe_verify_image(image: &str) -> Result<(), String> {
    if std::env::var("PERRY_CONTAINER_VERIFY_IMAGES")
        .ok()
        .as_deref()
        != Some("1")
    {
        return Ok(());
    }
    crate::container::verification::verify_image(image)
        .await
        .map(|_digest| ())
}

// ============ Container Lifecycle ============

/// Run a container from the given spec
#[no_mangle]
pub unsafe extern "C" fn js_container_run(spec_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec = match types::parse_container_spec(spec_ptr) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Err(e) = maybe_verify_image(&spec.image).await { return Err::<u64, String>(e); }
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        match backend.run(&spec).await {
            Ok(handle) => Ok(handle_to_promise_bits(types::register_container_handle(handle) as u64)),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

/// Create a container from the given spec without starting it
#[no_mangle]
pub unsafe extern "C" fn js_container_create(spec_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec = match types::parse_container_spec(spec_ptr) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Err(e) = maybe_verify_image(&spec.image).await { return Err::<u64, String>(e); }
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        match backend.create(&spec).await {
            Ok(handle) => Ok(handle_to_promise_bits(types::register_container_handle(handle) as u64)),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

/// Start a previously created container
#[no_mangle]
pub unsafe extern "C" fn js_container_start(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        backend.start(&id).await.map(|_| PROMISE_VOID_BITS).map_err(|e| e.to_string())
    });
    promise
}

/// Stop a running container
#[no_mangle]
pub unsafe extern "C" fn js_container_stop(id_ptr: *const StringHeader, timeout: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let timeout_opt = if timeout < 0.0 { None } else { Some(timeout as u32) };
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        backend.stop(&id, timeout_opt).await.map(|_| PROMISE_VOID_BITS).map_err(|e| e.to_string())
    });
    promise
}

/// Remove a container
#[no_mangle]
pub unsafe extern "C" fn js_container_remove(id_ptr: *const StringHeader, force: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        backend.remove(&id, force != 0.0).await.map(|_| PROMISE_VOID_BITS).map_err(|e| e.to_string())
    });
    promise
}

/// List containers
#[no_mangle]
pub unsafe extern "C" fn js_container_list(all: f64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        match backend.list(all != 0.0).await {
            Ok(containers) => Ok(handle_to_promise_bits(types::register_container_info_list(containers) as u64)),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

/// Inspect a container
#[no_mangle]
pub unsafe extern "C" fn js_container_inspect(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        match backend.inspect(&id).await {
            Ok(info) => Ok(handle_to_promise_bits(types::register_container_info(info) as u64)),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

/// Get logs from a container
#[no_mangle]
pub unsafe extern "C" fn js_container_logs(id_ptr: *const StringHeader, tail: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let tail_opt = if tail >= 0.0 { Some(tail as u32) } else { None };
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        match backend.logs(&id, tail_opt).await {
            Ok(logs) => Ok(handle_to_promise_bits(types::register_container_logs(logs) as u64)),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

/// Execute a command in a container
#[no_mangle]
pub unsafe extern "C" fn js_container_exec(
    id_ptr: *const StringHeader,
    cmd_json_ptr: *const StringHeader,
    env_json_ptr: *const StringHeader,
    workdir_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();
    let cmd_json = string_from_header(cmd_json_ptr);
    let env_json = string_from_header(env_json_ptr);
    let workdir = string_from_header(workdir_ptr);
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let cmd: Vec<String> = cmd_json.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let env: Option<HashMap<String, String>> = env_json.and_then(|s| serde_json::from_str(&s).ok());
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        match backend.exec(&id, &cmd, env.as_ref(), workdir.as_deref()).await {
            Ok(logs) => Ok(handle_to_promise_bits(types::register_container_logs(logs) as u64)),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

// ============ Image Management ============

/// Pull a container image
#[no_mangle]
pub unsafe extern "C" fn js_container_pullImage(reference_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let reference = string_from_header(reference_ptr).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Err(e) = maybe_verify_image(&reference).await { return Err::<u64, String>(e); }
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        backend.pull_image(&reference).await.map(|_| PROMISE_VOID_BITS).map_err(|e| e.to_string())
    });
    promise
}

/// List images
#[no_mangle]
pub unsafe extern "C" fn js_container_listImages() -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        match backend.list_images().await {
            Ok(images) => Ok(handle_to_promise_bits(types::register_image_info_list(images) as u64)),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

/// Build a container image
#[no_mangle]
pub unsafe extern "C" fn js_container_build(
    spec_ptr: *const StringHeader,
    image_name_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = string_from_header(spec_ptr).unwrap_or_else(|| "{}".to_string());
    let image_name = string_from_header(image_name_ptr).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let spec: perry_container_compose::types::ComposeServiceBuild = serde_json::from_str(&spec_json).map_err(|e| format!("Invalid build spec: {}", e))?;
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        backend.build(&spec, &image_name).await.map(|_| PROMISE_VOID_BITS).map_err(|e| e.to_string())
    });
    promise
}

/// Inspect an image
#[no_mangle]
pub unsafe extern "C" fn js_container_inspectImage(reference_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let reference = string_from_header(reference_ptr).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        match backend.inspect_image(&reference).await {
            Ok(info) => Ok(handle_to_promise_bits(types::register_image_info_list(vec![info]) as u64)),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

/// Remove an image
#[no_mangle]
pub unsafe extern "C" fn js_container_removeImage(reference_ptr: *const StringHeader, force: f64) -> *mut Promise {
    let promise = js_promise_new();
    let reference = string_from_header(reference_ptr).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        backend.remove_image(&reference, force != 0.0).await.map(|_| PROMISE_VOID_BITS).map_err(|e| e.to_string())
    });
    promise
}

// ============ Backend Info ============

/// Get the current backend name.
#[no_mangle]
pub unsafe extern "C" fn js_container_getBackend() -> *const StringHeader {
    if let Some(b) = BACKEND.get() {
        return string_to_js(b.backend_name());
    }
    let resolved = if let Ok(handle) = tokio::runtime::Handle::try_current() {
        match handle.runtime_flavor() {
            tokio::runtime::RuntimeFlavor::CurrentThread => None,
            _ => Some(tokio::task::block_in_place(|| handle.block_on(get_global_backend()))),
        }
    } else {
        tokio::runtime::Builder::new_current_thread().enable_all().build().ok().map(|rt| rt.block_on(get_global_backend()))
    };
    match resolved {
        Some(Ok(b)) => string_to_js(b.backend_name()),
        _ => string_to_js("unknown"),
    }
}

/// Detect backend and return probed info
#[no_mangle]
pub unsafe extern "C" fn js_container_detectBackend() -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise_deferred(
        promise as *mut u8,
        async move {
            match detect_backend().await {
                Ok(b) => Ok(serde_json::json!([{"name": b.backend_name(), "available": true, "reason": ""}]).to_string()),
                Err(e) => {
                    use perry_container_compose::error::ComposeError;
                    let json = match e {
                        ComposeError::NoBackendFound { probed } => serde_json::to_string(&probed).unwrap_or_else(|_| "[]".to_string()),
                        _ => serde_json::json!([{"name": "unknown", "available": false, "reason": e.to_string()}]).to_string(),
                    };
                    Ok(json)
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

// ============ Compose Functions ============

/// Bring up a Compose stack
#[no_mangle]
pub unsafe extern "C" fn js_container_composeUp(spec_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec = match types::parse_compose_spec(spec_ptr) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        let wrapper = compose::ComposeWrapper::new(spec, Arc::clone(backend));
        match wrapper.up().await {
            Ok(_) => Ok(handle_to_promise_bits(types::register_compose_handle(wrapper.engine().clone()))),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

/// Re-up services in an existing compose stack.
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_up(handle: f64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let handle_id = handle_id_from_f64(handle);
    let engine = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid compose handle".to_string()) });
            return promise;
        }
    };
    let services_json = string_from_header(services_json_ptr);
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let services: Vec<String> = services_json.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        engine.up(&services, true, false, false).await.map(|_| handle_to_promise_bits(handle_id as u64)).map_err(|e| e.to_string())
    });
    promise
}

/// Stop and remove compose stack.
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_down(handle: f64, opts_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let handle_id = handle_id_from_f64(handle);
    let opts_json = string_from_header(opts_ptr);
    let remove_volumes = opts_json.as_deref().and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok()).and_then(|v| v.get("volumes").and_then(|x| x.as_bool())).unwrap_or(false);
    let engine = match types::take_compose_handle(handle_id as u64) {
        Some(h) => h,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid compose handle".to_string()) });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let wrapper = compose::ComposeWrapper::new_from_engine(engine);
        wrapper.down(remove_volumes).await.map(|_| PROMISE_VOID_BITS).map_err(|e| e.to_string())
    });
    promise
}

/// Get process status of a compose stack.
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_ps(handle: f64) -> *mut Promise {
    let promise = js_promise_new();
    let handle_id = handle_id_from_f64(handle);
    let engine = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid compose handle".to_string()) });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let wrapper = compose::ComposeWrapper::new_from_engine(engine);
        match wrapper.ps().await {
            Ok(containers) => Ok(handle_to_promise_bits(types::register_container_info_list(containers) as u64)),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

/// Get logs from a compose stack.
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_logs(handle: f64, service_ptr: *const StringHeader, tail: f64) -> *mut Promise {
    let promise = js_promise_new();
    let handle_id = handle_id_from_f64(handle);
    let engine = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid compose handle".to_string()) });
            return promise;
        }
    };
    let service = string_from_header(service_ptr);
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let tail_opt = if tail.is_finite() && tail >= 0.0 { Some(tail as u32) } else { None };
        let wrapper = compose::ComposeWrapper::new_from_engine(engine);
        match wrapper.logs(service.as_deref(), tail_opt).await {
            Ok(logs) => Ok(handle_to_promise_bits(types::register_container_logs(logs) as u64)),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

/// Execute a command in a compose service.
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_exec(handle: f64, service_ptr: *const StringHeader, cmd_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let handle_id = handle_id_from_f64(handle);
    let engine = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid compose handle".to_string()) });
            return promise;
        }
    };
    let service = string_from_header(service_ptr).unwrap_or_default();
    let cmd_json = string_from_header(cmd_json_ptr);
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let cmd: Vec<String> = cmd_json.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let wrapper = compose::ComposeWrapper::new_from_engine(engine);
        match wrapper.exec(&service, &cmd).await {
            Ok(logs) => Ok(handle_to_promise_bits(types::register_container_logs(logs) as u64)),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

/// Start compose services.
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_start(handle: f64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let handle_id = handle_id_from_f64(handle);
    let engine = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid compose handle".to_string()) });
            return promise;
        }
    };
    let services_json = string_from_header(services_json_ptr);
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let services: Vec<String> = services_json.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        engine.start(&services).await.map(|_| PROMISE_VOID_BITS).map_err(|e| e.to_string())
    });
    promise
}

/// Stop compose services.
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_stop(handle: f64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let handle_id = handle_id_from_f64(handle);
    let engine = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid compose handle".to_string()) });
            return promise;
        }
    };
    let services_json = string_from_header(services_json_ptr);
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let services: Vec<String> = services_json.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        engine.stop(&services).await.map(|_| PROMISE_VOID_BITS).map_err(|e| e.to_string())
    });
    promise
}

/// Restart compose services.
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_restart(handle: f64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let handle_id = handle_id_from_f64(handle);
    let engine = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid compose handle".to_string()) });
            return promise;
        }
    };
    let services_json = string_from_header(services_json_ptr);
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let services: Vec<String> = services_json.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        engine.restart(&services).await.map(|_| PROMISE_VOID_BITS).map_err(|e| e.to_string())
    });
    promise
}

/// Get compose configuration.
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_config(handle: f64) -> *mut Promise {
    let promise = js_promise_new();
    let handle_id = handle_id_from_f64(handle);
    let engine = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid compose handle".to_string()) });
            return promise;
        }
    };
    crate::common::spawn_for_promise_deferred(
        promise as *mut u8,
        async move { engine.config().map_err(|e| e.to_string()) },
        |yaml| {
            let str_ptr = perry_runtime::js_string_from_bytes(yaml.as_ptr(), yaml.len() as u32);
            perry_runtime::JSValue::string_ptr(str_ptr).bits()
        },
    );
    promise
}

/// Get compose status.
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_status(handle: f64) -> *mut Promise {
    let promise = js_promise_new();
    let handle_id = handle_id_from_f64(handle);
    let engine = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid compose handle".to_string()) });
            return promise;
        }
    };
    crate::common::spawn_for_promise_deferred(
        promise as *mut u8,
        async move { Ok(engine.project_name.clone()) },
        |status| {
            let str_ptr = perry_runtime::js_string_from_bytes(status.as_ptr(), status.len() as u32);
            perry_runtime::JSValue::string_ptr(str_ptr).bits()
        },
    );
    promise
}

/// Get compose graph.
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_graph(handle: f64) -> *const StringHeader {
    let handle_id = handle_id_from_f64(handle);
    let engine = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h,
        None => return std::ptr::null(),
    };
    let json = serde_json::to_string(&engine.spec.services.keys().collect::<Vec<_>>()).unwrap_or_default();
    string_to_js(&json)
}

// Aliases for perry/compose
#[no_mangle] pub unsafe extern "C" fn js_compose_up(s: *const StringHeader) -> *mut Promise { js_container_composeUp(s) }
#[no_mangle] pub unsafe extern "C" fn js_compose_down(h: f64, o: *const StringHeader) -> *mut Promise { js_container_compose_down(h, o) }
#[no_mangle] pub unsafe extern "C" fn js_compose_ps(h: f64) -> *mut Promise { js_container_compose_ps(h) }
#[no_mangle] pub unsafe extern "C" fn js_compose_logs(h: f64, s: *const StringHeader, t: f64) -> *mut Promise { js_container_compose_logs(h, s, t) }
#[no_mangle] pub unsafe extern "C" fn js_compose_exec(h: f64, s: *const StringHeader, c: *const StringHeader) -> *mut Promise { js_container_compose_exec(h, s, c) }
#[no_mangle] pub unsafe extern "C" fn js_compose_config(h: f64) -> *mut Promise { js_container_compose_config(h) }
#[no_mangle] pub unsafe extern "C" fn js_compose_start(h: f64, s: *const StringHeader) -> *mut Promise { js_container_compose_start(h, s) }
#[no_mangle] pub unsafe extern "C" fn js_compose_stop(h: f64, s: *const StringHeader) -> *mut Promise { js_container_compose_stop(h, s) }
#[no_mangle] pub unsafe extern "C" fn js_compose_restart(h: f64, s: *const StringHeader) -> *mut Promise { js_container_compose_restart(h, s) }
#[no_mangle] pub unsafe extern "C" fn js_compose_status(h: f64) -> *mut Promise { js_container_compose_status(h) }
#[no_mangle] pub unsafe extern "C" fn js_compose_graph(h: f64) -> *const StringHeader { js_container_compose_graph(h) }

// ============ Workload Functions ============

#[no_mangle]
pub unsafe extern "C" fn js_workload_graph(name_ptr: *const StringHeader, nodes_json_ptr: *const StringHeader) -> *const StringHeader {
    let name = string_from_header(name_ptr).unwrap_or_default();
    let nodes_json = string_from_header(nodes_json_ptr).unwrap_or_else(|| "{}".to_string());
    let graph = perry_container_compose::WorkloadGraph { name, nodes: serde_json::from_str(&nodes_json).unwrap_or_default(), edges: vec![] };
    string_to_js(&serde_json::to_string(&graph).unwrap_or_default())
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_node(name_ptr: *const StringHeader, spec_json_ptr: *const StringHeader) -> *const StringHeader {
    let name = string_from_header(name_ptr).unwrap_or_default();
    let spec_json = string_from_header(spec_json_ptr).unwrap_or_else(|| "{}".to_string());
    let mut node: perry_container_compose::WorkloadNode = serde_json::from_str(&spec_json).unwrap_or_else(|_| perry_container_compose::WorkloadNode { id: name.clone(), name: name.clone(), image: None, resources: None, ports: vec![], env: HashMap::new(), depends_on: vec![], runtime: perry_container_compose::RuntimeSpec::Auto, policy: perry_container_compose::PolicySpec::default() });
    node.id = name.clone(); node.name = name;
    string_to_js(&serde_json::to_string(&node).unwrap_or_default())
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_runGraph(graph_ptr: *const StringHeader, opts_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let graph_json = string_from_header(graph_ptr).unwrap_or_default();
    let opts_json = string_from_header(opts_ptr).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let graph: perry_container_compose::WorkloadGraph = serde_json::from_str(&graph_json).map_err(|e| e.to_string())?;
        let opts: perry_container_compose::RunGraphOptions = serde_json::from_str(&opts_json).map_err(|e| e.to_string())?;
        let backend = get_global_backend().await.map_err(|e| e.to_string())?;
        let engine = Arc::new(perry_container_compose::WorkloadGraphEngine::new(graph, Arc::clone(backend)));
        match engine.run(opts).await {
            Ok(_) => Ok(handle_to_promise_bits(types::register_workload_handle(engine))),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_inspectGraph(handle: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id_from_f64(handle) as u64;
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let engine = types::WORKLOAD_HANDLES.get().and_then(|m| m.get(&id)).ok_or_else(|| "Invalid workload handle".to_string())?.clone();
        engine.status().await.map(|s| serde_json::to_string(&s).unwrap_or_default()).map_err(|e| e.to_string())
    }, |j| perry_runtime::JSValue::string_ptr(perry_runtime::js_string_from_bytes(j.as_ptr(), j.len() as u32)).bits());
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_down(id: i64, force: f64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = types::WORKLOAD_HANDLES.get().and_then(|m| m.get(&(id as u64))).ok_or_else(|| "Invalid workload handle".to_string())?.clone();
        engine.down(force != 0.0).await.map(|_| { if let Some(h) = types::WORKLOAD_HANDLES.get() { h.remove(&(id as u64)); } PROMISE_VOID_BITS }).map_err(|e| e.to_string())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_status(id: i64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let engine = types::WORKLOAD_HANDLES.get().and_then(|m| m.get(&(id as u64))).ok_or_else(|| "Invalid workload handle".to_string())?.clone();
        engine.status().await.map(|s| serde_json::to_string(&s).unwrap_or_default()).map_err(|e| e.to_string())
    }, |j| perry_runtime::JSValue::string_ptr(perry_runtime::js_string_from_bytes(j.as_ptr(), j.len() as u32)).bits());
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_logs(id: i64, node_ptr: *const StringHeader, tail: f64) -> *mut Promise {
    let promise = js_promise_new();
    let node_id = string_from_header(node_ptr).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let tail_opt = if tail >= 0.0 { Some(tail as u32) } else { None };
        let engine = types::WORKLOAD_HANDLES.get().and_then(|m| m.get(&(id as u64))).ok_or_else(|| "Invalid workload handle".to_string())?.clone();
        match engine.logs(&node_id, tail_opt).await {
            Ok(logs) => Ok(handle_to_promise_bits(types::register_container_logs(logs) as u64)),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_exec(id: i64, node_ptr: *const StringHeader, cmd_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let node_id = string_from_header(node_ptr).unwrap_or_default();
    let cmd_json = string_from_header(cmd_ptr).unwrap_or_else(|| "[]".to_string());
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let cmd: Vec<String> = serde_json::from_str(&cmd_json).unwrap_or_default();
        let engine = types::WORKLOAD_HANDLES.get().and_then(|m| m.get(&(id as u64))).ok_or_else(|| "Invalid workload handle".to_string())?.clone();
        match engine.exec(&node_id, &cmd).await {
            Ok(logs) => Ok(handle_to_promise_bits(types::register_container_logs(logs) as u64)),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_ps(id: i64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = types::WORKLOAD_HANDLES.get().and_then(|m| m.get(&(id as u64))).ok_or_else(|| "Invalid workload handle".to_string())?.clone();
        match engine.ps().await {
            Ok(infos) => Ok(handle_to_promise_bits(types::register_container_info_list(infos.into_iter().map(|i| ContainerInfo { id: i.container_id.unwrap_or_default(), name: i.name, image: i.image.unwrap_or_default(), status: format!("{:?}", i.state), ports: vec![], labels: HashMap::new(), created: "".to_string(), ip_address: i.ip_address.unwrap_or_default() }).collect()) as u64)),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_graph(id: i64) -> *const StringHeader {
    let engine = match types::WORKLOAD_HANDLES.get().and_then(|m| m.get(&(id as u64))) { Some(e) => e.clone(), None => return std::ptr::null() };
    string_to_js(&serde_json::to_string(&engine.graph).unwrap_or_default())
}

// ============ Module Initialization ============

#[no_mangle]
pub extern "C" fn js_container_module_init() {
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.spawn(async { let _ = get_global_backend().await; });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn module_init_safe() { js_container_module_init(); }
}
