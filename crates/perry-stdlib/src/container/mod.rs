//! Perry container module FFI bridge.

pub mod backend;
pub mod capability;
pub mod compose;
pub mod workload;
pub mod types;
pub mod verification;
pub use mod_private::ContainerError;

use perry_container_compose::backend::{detect_backend, ContainerBackend};
use perry_container_compose::error::compose_error_to_js;
use perry_container_compose::error::ComposeError;
use perry_container_compose::ComposeEngine;
use perry_container_compose::workload::WorkloadGraphEngine;
use perry_runtime::{js_promise_new, Promise, StringHeader, JSValue, js_string_from_bytes};
use std::sync::{Arc, OnceLock};
use std::sync::atomic::Ordering;
use crate::container::types::*;
use crate::common::spawn_for_promise_deferred;
use dashmap::DashMap;

pub(crate) mod mod_private {
    use super::*;
    use thiserror::Error;

    #[derive(Debug, Error)]
    pub enum ContainerError {
        #[error("Not found: {0}")]
        NotFound(String),
        #[error("Backend error (exit {code}): {message}")]
        BackendError { code: i32, message: String },
        #[error("Image verification failed for '{image}': {reason}")]
        VerificationFailed { image: String, reason: String },
        #[error("Dependency cycle: {cycle:?}")]
        DependencyCycle { cycle: Vec<String> },
        #[error("Service '{service}' failed to start: {error}")]
        ServiceStartupFailed { service: String, error: String },
        #[error("Invalid configuration: {0}")]
        InvalidConfig(String),
    }

    impl From<perry_container_compose::error::ComposeError> for ContainerError {
        fn from(e: perry_container_compose::error::ComposeError) -> Self {
            match e {
                perry_container_compose::error::ComposeError::NotFound(s) => ContainerError::NotFound(s),
                perry_container_compose::error::ComposeError::BackendError { code, message } => ContainerError::BackendError { code, message },
                perry_container_compose::error::ComposeError::VerificationFailed { image, reason } => ContainerError::VerificationFailed { image, reason },
                perry_container_compose::error::ComposeError::DependencyCycle { services } => ContainerError::DependencyCycle { cycle: services },
                perry_container_compose::error::ComposeError::ServiceStartupFailed { service, message } => ContainerError::ServiceStartupFailed { service, error: message },
                other => ContainerError::InvalidConfig(other.to_string()),
            }
        }
    }

    use tokio::sync::Mutex;

    pub static BACKEND: OnceLock<Arc<dyn ContainerBackend + Send + Sync>> = OnceLock::new();
    static INIT_MUTEX: Mutex<()> = Mutex::const_new(());

    pub async fn get_global_backend_instance() -> Result<Arc<dyn ContainerBackend + Send + Sync>, String> {
        if let Some(b) = BACKEND.get() {
            return Ok(Arc::clone(b));
        }

        let _guard = INIT_MUTEX.lock().await;
        if let Some(b) = BACKEND.get() {
            return Ok(Arc::clone(b));
        }

        let backend_res = detect_backend().await;

        match backend_res {
            Ok(b) => {
                let _ = BACKEND.set(Arc::clone(&b));
                Ok(b)
            }
            Err(probed) => Err(format!("No backend found: {:?}", probed)),
        }
    }
}

use mod_private::get_global_backend_instance;

#[no_mangle]
pub unsafe extern "C" fn js_container_run(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            let err = compose_error_to_js(&ComposeError::NotFound("Invalid spec JSON".to_string()));
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(err) });
            return promise;
        }
    };

    let spec: ContainerSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            let err = compose_error_to_js(&ComposeError::validation(format!("Invalid ContainerSpec: {}", e)));
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(err) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| compose_error_to_js(&ComposeError::BackendNotAvailable { name: "unknown".to_string(), reason: e }))?;
        let handle = backend.run(&spec).await.map_err(|e| compose_error_to_js(&e))?;
        let id = register_container_handle(handle);
        Ok(id)
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_create(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            let err = compose_error_to_js(&ComposeError::NotFound("Invalid spec JSON".to_string()));
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(err) });
            return promise;
        }
    };

    let spec: ContainerSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            let err = compose_error_to_js(&ComposeError::validation(format!("Invalid ContainerSpec: {}", e)));
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(err) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| compose_error_to_js(&ComposeError::BackendNotAvailable { name: "unknown".to_string(), reason: e }))?;
        let handle = backend.create(&spec).await.map_err(|e| compose_error_to_js(&e))?;
        let id = register_container_handle(handle);
        Ok(id)
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_start(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            let err = compose_error_to_js(&ComposeError::NotFound("Invalid ID".to_string()));
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(err) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| compose_error_to_js(&ComposeError::BackendNotAvailable { name: "unknown".to_string(), reason: e }))?;
        backend.start(&id).await.map_err(|e| compose_error_to_js(&e))?;
        Ok(0)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_stop(id_ptr: *const StringHeader, opts_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            let err = compose_error_to_js(&ComposeError::NotFound("Invalid ID".to_string()));
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(err) });
            return promise;
        }
    };
    let timeout = if !opts_json_ptr.is_null() && (opts_json_ptr as usize) >= 0x1000 {
        let opts_json = string_from_header(opts_json_ptr).unwrap_or_default();
        let opts: serde_json::Value = serde_json::from_str(&opts_json).unwrap_or(serde_json::Value::Null);
        opts.get("timeout").and_then(|v| v.as_u64()).map(|v| v as u32)
    } else {
        None
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| compose_error_to_js(&ComposeError::BackendNotAvailable { name: "unknown".to_string(), reason: e }))?;
        backend.stop(&id, timeout).await.map_err(|e| compose_error_to_js(&e))?;
        Ok(0)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_remove(id_ptr: *const StringHeader, opts_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            let err = compose_error_to_js(&ComposeError::NotFound("Invalid ID".to_string()));
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(err) });
            return promise;
        }
    };
    let force = if !opts_json_ptr.is_null() && (opts_json_ptr as usize) >= 0x1000 {
        let opts_json = string_from_header(opts_json_ptr).unwrap_or_default();
        let opts: serde_json::Value = serde_json::from_str(&opts_json).unwrap_or(serde_json::Value::Null);
        opts.get("force").and_then(|v| v.as_bool()).unwrap_or(false)
    } else {
        false
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| compose_error_to_js(&ComposeError::BackendNotAvailable { name: "unknown".to_string(), reason: e }))?;
        backend.remove(&id, force).await.map_err(|e| compose_error_to_js(&e))?;
        Ok(0)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_list(opts_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let all = if !opts_json_ptr.is_null() && (opts_json_ptr as usize) >= 0x1000 {
        let opts_json = string_from_header(opts_json_ptr).unwrap_or_default();
        let opts: serde_json::Value = serde_json::from_str(&opts_json).unwrap_or(serde_json::Value::Null);
        opts.get("all").and_then(|v| v.as_bool()).unwrap_or(false)
    } else {
        false
    };

    spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| compose_error_to_js(&ComposeError::BackendNotAvailable { name: "unknown".to_string(), reason: e }))?;
        backend.list(all).await.map_err(|e| compose_error_to_js(&e))
    }, |list| {
        let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
        CONTAINER_INFO_LIST_HANDLES.get_or_init(DashMap::new).insert(id, list);
        id
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_inspect(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            let err = compose_error_to_js(&ComposeError::NotFound("Invalid ID".to_string()));
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(err) });
            return promise;
        }
    };

    spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| compose_error_to_js(&ComposeError::BackendNotAvailable { name: "unknown".to_string(), reason: e }))?;
        backend.inspect(&id).await.map_err(|e| compose_error_to_js(&e))
    }, |info| {
        let json = serde_json::to_string(&info).unwrap_or_else(|_| "{}".to_string());
        let str_ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_logs(id_ptr: *const StringHeader, opts_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            let err = compose_error_to_js(&ComposeError::NotFound("Invalid ID".to_string()));
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(err) });
            return promise;
        }
    };
    let tail = if !opts_json_ptr.is_null() && (opts_json_ptr as usize) >= 0x1000 {
        let opts_json = string_from_header(opts_json_ptr).unwrap_or_default();
        let opts: serde_json::Value = serde_json::from_str(&opts_json).unwrap_or(serde_json::Value::Null);
        opts.get("tail").and_then(|v| v.as_u64()).map(|v| v as u32)
    } else {
        None
    };

    spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| compose_error_to_js(&ComposeError::BackendNotAvailable { name: "unknown".to_string(), reason: e }))?;
        backend.logs(&id, tail).await.map_err(|e| compose_error_to_js(&e))
    }, |logs| {
        let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
        CONTAINER_LOGS_HANDLES.get_or_init(DashMap::new).insert(id, logs);
        id
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_exec(
    id_ptr: *const StringHeader,
    cmd_json_ptr: *const StringHeader,
    opts_json_ptr: *const StringHeader
) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            let err = compose_error_to_js(&ComposeError::NotFound("Invalid ID".to_string()));
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(err) });
            return promise;
        }
    };
    let cmd: Vec<String> = match string_from_header(cmd_json_ptr).and_then(|s| serde_json::from_str(&s).ok()) {
        Some(v) => v,
        None => {
            let err = compose_error_to_js(&ComposeError::validation("Invalid cmd JSON".to_string()));
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(err) });
            return promise;
        }
    };

    let opts: serde_json::Value = string_from_header(opts_json_ptr)
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::Value::Null);

    let env: Option<std::collections::HashMap<String, String>> = opts.get("env")
        .and_then(|v| serde_json::from_value(v.clone()).ok());
    let workdir = opts.get("workdir")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| compose_error_to_js(&ComposeError::BackendNotAvailable { name: "unknown".to_string(), reason: e }))?;
        backend.exec(&id, &cmd, env.as_ref(), workdir.as_deref()).await.map_err(|e| compose_error_to_js(&e))
    }, |logs| {
        let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
        CONTAINER_LOGS_HANDLES.get_or_init(DashMap::new).insert(id, logs);
        id
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_build(spec_json_ptr: *const StringHeader, image_name_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            let err = compose_error_to_js(&ComposeError::NotFound("Invalid spec JSON".to_string()));
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(err) });
            return promise;
        }
    };
    let image_name = match string_from_header(image_name_ptr) {
        Some(s) => s,
        None => {
            let err = compose_error_to_js(&ComposeError::NotFound("Invalid image name".to_string()));
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(err) });
            return promise;
        }
    };

    let spec: ComposeServiceBuild = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            let err = compose_error_to_js(&ComposeError::validation(format!("Invalid build spec: {}", e)));
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(err) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| compose_error_to_js(&ComposeError::BackendNotAvailable { name: "unknown".to_string(), reason: e }))?;
        backend.build(&spec, &image_name).await.map_err(|e| compose_error_to_js(&e))?;
        Ok(0)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_pullImage(ref_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let reference = match string_from_header(ref_ptr) {
        Some(s) => s,
        None => {
            let err = compose_error_to_js(&ComposeError::NotFound("Invalid image ref".to_string()));
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(err) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| compose_error_to_js(&ComposeError::BackendNotAvailable { name: "unknown".to_string(), reason: e }))?;
        backend.pull_image(&reference).await.map_err(|e| compose_error_to_js(&e))?;
        Ok(0)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_listImages() -> *mut Promise {
    let promise = js_promise_new();
    spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| compose_error_to_js(&ComposeError::BackendNotAvailable { name: "unknown".to_string(), reason: e }))?;
        backend.list_images().await.map_err(|e| compose_error_to_js(&e))
    }, |list| {
        let json = serde_json::to_string(&list).unwrap_or_else(|_| "[]".to_string());
        let str_ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_removeImage(ref_ptr: *const StringHeader, force: f64) -> *mut Promise {
    let promise = js_promise_new();
    let reference = match string_from_header(ref_ptr) {
        Some(s) => s,
        None => {
            let err = compose_error_to_js(&ComposeError::NotFound("Invalid image ref".to_string()));
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(err) });
            return promise;
        }
    };
    let f = force != 0.0;

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| compose_error_to_js(&ComposeError::BackendNotAvailable { name: "unknown".to_string(), reason: e }))?;
        backend.remove_image(&reference, f).await.map_err(|e| compose_error_to_js(&e))?;
        Ok(0)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_getBackend() -> *const StringHeader {
    let name = if let Some(backend) = mod_private::BACKEND.get() {
        backend.backend_name()
    } else {
        "unknown"
    };
    js_string_from_bytes(name.as_ptr(), name.len() as u32)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_detectBackend() -> *mut Promise {
    let promise = js_promise_new();
    spawn_for_promise_deferred(promise as *mut u8, async move {
        match detect_backend().await {
            Ok(backend) => {
                let name = backend.backend_name().to_string();
                let _ = mod_private::BACKEND.set(Arc::clone(&backend));
                Ok(vec![perry_container_compose::error::BackendProbeResult {
                    name,
                    available: true,
                    reason: String::new(),
                }])
            }
            Err(probed) => Ok(probed),
        }
    }, |probed| {
        let json = serde_json::to_string(&probed).unwrap_or_else(|_| "[]".to_string());
        let str_ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_module_init() {
    crate::common::spawn(async move {
        let _ = get_global_backend_instance().await;
    });
}

// Compose FFI

#[no_mangle]
pub unsafe extern "C" fn js_container_composeUp(spec_or_path_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_or_path = match string_from_header(spec_or_path_ptr) {
        Some(s) => s,
        None => {
            let err = compose_error_to_js(&ComposeError::NotFound("Invalid pointer".to_string()));
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(err) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = if spec_or_path.trim_start().starts_with('{') {
            let spec: perry_container_compose::types::ComposeSpec = serde_json::from_str(&spec_or_path).map_err(|e| compose_error_to_js(&ComposeError::validation(format!("Invalid ComposeSpec: {}", e))))?;
            let backend = get_global_backend_instance().await.map_err(|e| compose_error_to_js(&ComposeError::BackendNotAvailable { name: "unknown".to_string(), reason: e }))?;
            let project_name = spec.name.clone().unwrap_or_else(|| "default".to_string());
            ComposeEngine::new(spec, project_name, backend)
        } else {
             let project = perry_container_compose::project::ComposeProject::load_from_files(&[std::path::PathBuf::from(&spec_or_path)], None, &[])
                 .map_err(|e| compose_error_to_js(&ComposeError::validation(format!("Failed to load project: {}", e))))?;
             let backend = get_global_backend_instance().await.map_err(|e| compose_error_to_js(&ComposeError::BackendNotAvailable { name: "unknown".to_string(), reason: e }))?;
             ComposeEngine::new(project.spec, project.project_name, backend)
        };

        let handle = register_compose_handle(engine);
        let arc_engine = {
            let handles = COMPOSE_HANDLES.get_or_init(DashMap::new);
            handles.get(&handle).map(|e| Arc::clone(&e.0)).unwrap()
        };

        arc_engine.up(&[], true, false, false).await.map_err(|e| compose_error_to_js(&e))?;
        Ok(handle)
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_down(handle_id: f64, volumes: i32) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    let v = volumes != 0;
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = {
            let handles = COMPOSE_HANDLES.get_or_init(DashMap::new);
            handles.get(&id).map(|e| Arc::clone(&e.0))
        }.ok_or_else(|| compose_error_to_js(&ComposeError::NotFound(format!("Compose stack {} not found", id))))?;

        engine.down(&[], false, v).await.map_err(|e| compose_error_to_js(&e))?;
        COMPOSE_HANDLES.get_or_init(DashMap::new).remove(&id);
        Ok(0)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_ps(handle_id: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    spawn_for_promise_deferred(promise as *mut u8, async move {
        let engine = {
            let handles = COMPOSE_HANDLES.get_or_init(DashMap::new);
            handles.get(&id).map(|e| Arc::clone(&e.0))
        }.ok_or_else(|| compose_error_to_js(&ComposeError::NotFound(format!("Compose stack {} not found", id))))?;
        engine.ps().await.map_err(|e| compose_error_to_js(&e))
    }, |list| {
        let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
        CONTAINER_INFO_LIST_HANDLES.get_or_init(DashMap::new).insert(id, list);
        id
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_logs(handle_id: f64, service_ptr: *const StringHeader, tail: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    let service = string_from_header(service_ptr);
    let t = if tail >= 0.0 { Some(tail as u32) } else { None };

    spawn_for_promise_deferred(promise as *mut u8, async move {
        let engine = {
            let handles = COMPOSE_HANDLES.get_or_init(DashMap::new);
            handles.get(&id).map(|e| Arc::clone(&e.0))
        }.ok_or_else(|| compose_error_to_js(&ComposeError::NotFound(format!("Compose stack {} not found", id))))?;
        let services = service.map(|s| vec![s]).unwrap_or_default();
        engine.logs(&services, t).await.map_err(|e| compose_error_to_js(&e))
    }, |logs_map| {
        let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
        let mut combined_stdout = String::new();
        let mut combined_stderr = String::new();
        for (svc, logs) in logs_map {
            combined_stdout.push_str(&format!("[{}] {}", svc, logs.stdout));
            combined_stderr.push_str(&format!("[{}] {}", svc, logs.stderr));
        }
        CONTAINER_LOGS_HANDLES.get_or_init(DashMap::new).insert(id, ContainerLogs { stdout: combined_stdout, stderr: combined_stderr });
        id
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_exec(
    handle_id: f64,
    service_ptr: *const StringHeader,
    cmd_json_ptr: *const StringHeader,
    opts_json_ptr: *const StringHeader
) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    let service = match string_from_header(service_ptr) {
        Some(s) => s,
        None => {
            let err = compose_error_to_js(&ComposeError::NotFound("Invalid service name".to_string()));
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(err) });
            return promise;
        }
    };
    let cmd: Vec<String> = match string_from_header(cmd_json_ptr).and_then(|s| serde_json::from_str(&s).ok()) {
        Some(v) => v,
        None => {
            let err = compose_error_to_js(&ComposeError::validation("Invalid cmd JSON".to_string()));
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(err) });
            return promise;
        }
    };

    let opts: serde_json::Value = string_from_header(opts_json_ptr)
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::Value::Null);

    let env: Option<std::collections::HashMap<String, String>> = opts.get("env")
        .and_then(|v| serde_json::from_value(v.clone()).ok());
    let workdir = opts.get("workdir")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    spawn_for_promise_deferred(promise as *mut u8, async move {
        let engine = {
            let handles = COMPOSE_HANDLES.get_or_init(DashMap::new);
            handles.get(&id).map(|e| Arc::clone(&e.0))
        }.ok_or_else(|| compose_error_to_js(&ComposeError::NotFound(format!("Compose stack {} not found", id))))?;

        let svc = engine.spec.services.get(&service).ok_or_else(|| compose_error_to_js(&ComposeError::NotFound(format!("Service {} not found", service))))?;
        let container_name = perry_container_compose::service::service_container_name(svc, &service);
        engine.backend.exec(&container_name, &cmd, env.as_ref(), workdir.as_deref()).await.map_err(|e| compose_error_to_js(&e))
    }, |logs| {
        let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
        CONTAINER_LOGS_HANDLES.get_or_init(DashMap::new).insert(id, logs);
        id
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_config(handle_id: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    spawn_for_promise_deferred(promise as *mut u8, async move {
        let engine = {
            let handles = COMPOSE_HANDLES.get_or_init(DashMap::new);
            handles.get(&id).map(|e| Arc::clone(&e.0))
        }.ok_or_else(|| compose_error_to_js(&ComposeError::NotFound(format!("Compose stack {} not found", id))))?;
        engine.config().map_err(|e| compose_error_to_js(&e))
    }, |config| {
        let str_ptr = js_string_from_bytes(config.as_ptr(), config.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_start(handle_id: f64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    let services: Vec<String> = string_from_header(services_json_ptr).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = {
            let handles = COMPOSE_HANDLES.get_or_init(DashMap::new);
            handles.get(&id).map(|e| Arc::clone(&e.0))
        }.ok_or_else(|| compose_error_to_js(&ComposeError::NotFound(format!("Compose stack {} not found", id))))?;
        engine.start(&services).await.map(|_| 0).map_err(|e| compose_error_to_js(&e))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_stop(handle_id: f64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    let services: Vec<String> = string_from_header(services_json_ptr).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = {
            let handles = COMPOSE_HANDLES.get_or_init(DashMap::new);
            handles.get(&id).map(|e| Arc::clone(&e.0))
        }.ok_or_else(|| compose_error_to_js(&ComposeError::NotFound(format!("Compose stack {} not found", id))))?;
        engine.stop(&services).await.map(|_| 0).map_err(|e| compose_error_to_js(&e))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_restart(handle_id: f64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    let services: Vec<String> = string_from_header(services_json_ptr).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = {
            let handles = COMPOSE_HANDLES.get_or_init(DashMap::new);
            handles.get(&id).map(|e| Arc::clone(&e.0))
        }.ok_or_else(|| compose_error_to_js(&ComposeError::NotFound(format!("Compose stack {} not found", id))))?;
        engine.restart(&services).await.map(|_| 0).map_err(|e| compose_error_to_js(&e))
    });
    promise
}

// Workload FFI

// Workload stateful builder registry
static WORKLOAD_BUILDER_HANDLES: OnceLock<DashMap<u64, WorkloadGraph>> = OnceLock::new();

fn register_workload_builder(graph: WorkloadGraph) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    WORKLOAD_BUILDER_HANDLES.get_or_init(DashMap::new).insert(id, graph);
    id
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_graph(name_ptr: *const StringHeader) -> f64 {
    let name = string_from_header(name_ptr).unwrap_or_else(|| "default".to_string());
    let graph = WorkloadGraph {
        name,
        nodes: indexmap::IndexMap::new(),
        edges: Vec::new(),
    };
    register_workload_builder(graph) as f64
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_node(handle_id: f64, id_ptr: *const StringHeader, spec_json_ptr: *const StringHeader) -> f64 {
    let hid = handle_id as u64;
    let node_id = string_from_header(id_ptr).unwrap_or_default();
    let spec_json = string_from_header(spec_json_ptr).unwrap_or_default();
    let node: WorkloadNode = serde_json::from_str(&spec_json).unwrap_or_else(|_| WorkloadNode::default());

    if let Some(mut graph) = WORKLOAD_BUILDER_HANDLES.get_or_init(DashMap::new).get_mut(&hid) {
        graph.nodes.insert(node_id, node);
    }
    handle_id
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_runGraph(handle_id: f64, opts_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let hid = handle_id as u64;
    let opts_json = string_from_header(opts_json_ptr).unwrap_or_default();
    let opts: RunGraphOptions = serde_json::from_str(&opts_json).unwrap_or_else(|_| RunGraphOptions::default());

    let graph = {
        let handles = WORKLOAD_BUILDER_HANDLES.get_or_init(DashMap::new);
        handles.remove(&hid).map(|(_, g)| g)
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let g = graph.ok_or_else(|| compose_error_to_js(&ComposeError::NotFound(format!("Workload builder {} not found", hid))))?;
        let backend = get_global_backend_instance().await.map_err(|e| compose_error_to_js(&ComposeError::BackendNotAvailable { name: "unknown".to_string(), reason: e }))?;
        let engine = WorkloadGraphEngine::new(backend);
        let handle = engine.run_graph(&g, &opts).await.map_err(|e| compose_error_to_js(&e))?;
        Ok(handle.stack_id)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_inspectGraph(handle_id: f64) -> *mut Promise {
    let promise = js_promise_new();
    let hid = handle_id as u64;

    spawn_for_promise_deferred(promise as *mut u8, async move {
        let engine = {
            let handles = COMPOSE_HANDLES.get_or_init(DashMap::new);
            handles.get(&hid).map(|e| Arc::clone(&e.0))
        }.ok_or_else(|| compose_error_to_js(&ComposeError::NotFound(format!("Workload stack {} not found", hid))))?;

        engine.status().await.map_err(|e| compose_error_to_js(&e))
    }, |status| {
        let json = serde_json::to_string(&status).unwrap_or_else(|_| "{}".to_string());
        let str_ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_down(handle_id: f64, volumes: f64) -> *mut Promise {
    js_container_compose_down(handle_id, volumes as i32)
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_status(handle_id: f64) -> *mut Promise {
    js_workload_inspectGraph(handle_id)
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_graph(handle_id: f64) -> *const StringHeader {
    let hid = handle_id as u64;
    let config = {
        let handles = COMPOSE_HANDLES.get_or_init(DashMap::new);
        handles.get(&hid).and_then(|e| e.0.config().ok())
    }.unwrap_or_default();
    js_string_from_bytes(config.as_ptr(), config.len() as u32)
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_logs(handle_id: f64, service_ptr: *const StringHeader, tail: f64) -> *mut Promise {
    js_container_compose_logs(handle_id, service_ptr, tail)
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_exec(handle_id: f64, service_ptr: *const StringHeader, cmd_json_ptr: *const StringHeader, opts_json_ptr: *const StringHeader) -> *mut Promise {
    js_container_compose_exec(handle_id, service_ptr, cmd_json_ptr, opts_json_ptr)
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_ps(handle_id: f64) -> *mut Promise {
    js_container_compose_ps(handle_id)
}
