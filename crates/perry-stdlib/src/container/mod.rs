//! Container module for Perry
//!
//! Provides OCI container management with platform-adaptive backend selection.

pub mod backend;
pub mod capability;
pub mod compose;
pub mod types;

pub use perry_container_compose::types::{ContainerSpec, ComposeSpec, ComposeService, ComposeNetwork, ComposeVolume, ComposeSecret, ComposeConfigObj, ListOrDict, DependsOnSpec, DependsOnCondition, ComposeDependsOn, ContainerInfo, ContainerLogs, ImageInfo};
pub mod verification;

pub use types::*; // Re-export types to be visible at perry_stdlib::container::*

use perry_runtime::{js_promise_new, Promise, StringHeader};
use perry_container_compose::backend::{detect_backend, ContainerBackend};
use std::sync::{Arc, OnceLock};

/// Registry for global backend instance
static BACKEND: OnceLock<Arc<dyn ContainerBackend>> = OnceLock::new();

/// Get or initialize the global backend instance.
/// If PERRY_CONTAINER_BACKEND is set, it will be used.
pub async fn get_backend_instance() -> Result<Arc<dyn ContainerBackend>, String> {
    if let Some(b) = BACKEND.get() {
        return Ok(Arc::clone(b));
    }

    let b = detect_backend().await.map_err(|e| e.to_string())?;
    let arc: Arc<dyn ContainerBackend> = Arc::new(b);
    // Best effort set, ignore if already set by another thread
    let _ = BACKEND.set(Arc::clone(&arc));
    Ok(arc)
}

// ============ Container API ============

#[no_mangle]
pub unsafe extern "C" fn js_container_run(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = match types::string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid spec JSON".into())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let spec: ContainerSpec = serde_json::from_str(&spec_json).map_err(|e| e.to_string())?;
        let backend = get_backend_instance().await?;
        let handle = backend.run(&spec).await.map_err(|e| e.to_string())?;
        Ok(types::register_container_handle(handle))
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_create(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = match types::string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid spec JSON".into())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let spec: ContainerSpec = serde_json::from_str(&spec_json).map_err(|e| e.to_string())?;
        let backend = get_backend_instance().await?;
        let handle = backend.create(&spec).await.map_err(|e| e.to_string())?;
        Ok(types::register_container_handle(handle))
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_start(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = match types::string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid ID".into())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_backend_instance().await?;
        backend.start(&id).await.map_err(|e| e.to_string())?;
        Ok(0u64)
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_stop(id_ptr: *const StringHeader, timeout: i32) -> *mut Promise {
    let promise = js_promise_new();
    let id = match types::string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid ID".into())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_backend_instance().await?;
        let t = if timeout < 0 { None } else { Some(timeout as u32) };
        backend.stop(&id, t).await.map_err(|e| e.to_string())?;
        Ok(0u64)
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_remove(id_ptr: *const StringHeader, force: i32) -> *mut Promise {
    let promise = js_promise_new();
    let id = match types::string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid ID".into())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_backend_instance().await?;
        backend.remove(&id, force != 0).await.map_err(|e| e.to_string())?;
        Ok(0u64)
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_list(all: i32) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_backend_instance().await?;
        let list = backend.list(all != 0).await.map_err(|e| e.to_string())?;
        Ok(types::register_container_info_list(list))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_inspect(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = match types::string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid ID".into())
            });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_backend_instance().await?;
        let info = backend.inspect(&id).await.map_err(|e| e.to_string())?;
        Ok(types::register_container_info(info))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_logs(id_ptr: *const StringHeader, tail: i32) -> *mut Promise {
    let promise = js_promise_new();
    let id = match types::string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid ID".into())
            });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_backend_instance().await?;
        let t = if tail < 0 { None } else { Some(tail as u32) };
        let logs = backend.logs(&id, t).await.map_err(|e| e.to_string())?;
        Ok(types::register_container_logs(logs))
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
    let id = types::string_from_header(id_ptr).unwrap_or_default();
    let cmd_json = types::string_from_header(cmd_json_ptr).unwrap_or_default();
    let env_json = types::string_from_header(env_json_ptr).unwrap_or_default();
    let workdir = types::string_from_header(workdir_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let cmd: Vec<String> = serde_json::from_str(&cmd_json).unwrap_or_default();
        let env: Option<std::collections::HashMap<String, String>> = serde_json::from_str(&env_json).ok();
        let backend = get_backend_instance().await?;
        let logs = backend.exec(&id, &cmd, env.as_ref(), workdir.as_deref()).await.map_err(|e| e.to_string())?;
        Ok(types::register_container_logs(logs))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_pullImage(ref_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let reference = types::string_from_header(ref_ptr).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_backend_instance().await?;
        backend.pull_image(&reference).await.map_err(|e| e.to_string())?;
        Ok(0u64)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_listImages() -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_backend_instance().await?;
        let list = backend.list_images().await.map_err(|e| e.to_string())?;
        Ok(types::register_image_info_list(list))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_removeImage(ref_ptr: *const StringHeader, force: i32) -> *mut Promise {
    let promise = js_promise_new();
    let reference = types::string_from_header(ref_ptr).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_backend_instance().await?;
        backend.remove_image(&reference, force != 0).await.map_err(|e| e.to_string())?;
        Ok(0u64)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_getBackend() -> *const StringHeader {
    let name = BACKEND.get().map(|b| b.backend_name()).unwrap_or("not-initialized");
    let bytes = name.as_bytes();
    perry_runtime::js_string_from_bytes(bytes.as_ptr(), bytes.len() as u32)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_detectBackend() -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match detect_backend().await {
            Ok(b) => {
                let name = b.backend_name().to_string();
                let bytes = name.as_bytes();
                let h = perry_runtime::js_string_from_bytes(bytes.as_ptr(), bytes.len() as u32);
                Ok(perry_runtime::js_nanbox_string(h as i64).to_bits())
            }
            Err(probed) => {
                let json = serde_json::to_string(&probed).unwrap_or_default();
                Err::<u64, String>(json)
            }
        }
    });
    promise
}

// ============ Compose API ============

#[no_mangle]
pub unsafe extern "C" fn js_container_composeUp(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = match types::string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid spec JSON".into())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let spec: ComposeSpec = serde_json::from_str(&spec_json).map_err(|e| e.to_string())?;
        let backend = get_backend_instance().await?;
        let project_name = spec.name.clone().unwrap_or_else(|| "default".into());
        let engine = perry_container_compose::compose::ComposeEngine::new(spec, project_name, backend);
        let arc_engine = Arc::new(engine);
        let _handle = arc_engine.up(&[], true, false, false).await.map_err(|e| e.to_string())?;
        let engine_id = types::register_compose_engine(arc_engine);
        Ok(engine_id)
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_up(spec_json_ptr: *const StringHeader) -> *mut Promise {
    js_container_composeUp(spec_json_ptr)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_down(engine_id: u64, volumes: i32) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = types::get_compose_engine(engine_id).ok_or("Invalid engine handle")?;
        engine.down(volumes != 0, false).await.map_err(|e| e.to_string())?;
        types::take_compose_engine(engine_id);
        Ok(0u64)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_ps(engine_id: u64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = types::get_compose_engine(engine_id).ok_or("Invalid engine handle")?;
        let list = engine.ps().await.map_err(|e| e.to_string())?;
        Ok(types::register_container_info_list(list))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_logs(engine_id: u64, service_ptr: *const StringHeader, tail: i32) -> *mut Promise {
    let promise = js_promise_new();
    let service = types::string_from_header(service_ptr);
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = types::get_compose_engine(engine_id).ok_or("Invalid engine handle")?;
        let t = if tail < 0 { None } else { Some(tail as u32) };
        let logs = engine.logs(service.as_deref(), t).await.map_err(|e| e.to_string())?;
        Ok(types::register_container_logs(logs))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_exec(
    engine_id: u64,
    service_ptr: *const StringHeader,
    cmd_json_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let service = types::string_from_header(service_ptr).unwrap_or_default();
    let cmd_json = types::string_from_header(cmd_json_ptr).unwrap_or_default();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let cmd: Vec<String> = serde_json::from_str(&cmd_json).unwrap_or_default();
        let engine = types::get_compose_engine(engine_id).ok_or("Invalid engine handle")?;
        let logs = engine.exec(&service, &cmd).await.map_err(|e| e.to_string())?;
        Ok(types::register_container_logs(logs))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_config(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = types::string_from_header(spec_json_ptr).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let spec: ComposeSpec = serde_json::from_str(&spec_json).map_err(|e| e.to_string())?;
        // Validate and return resolved config as JSON
        let json = serde_json::to_string(&spec).map_err(|e| e.to_string())?;
        let bytes = json.as_bytes();
        let h = perry_runtime::js_string_from_bytes(bytes.as_ptr(), bytes.len() as u32);

        // Resolve with parsed JSON array
        let parsed = perry_runtime::json::js_json_parse(h);
        Ok(parsed.bits())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_start(engine_id: u64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let services_json = types::string_from_header(services_json_ptr).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let services: Vec<String> = serde_json::from_str(&services_json).unwrap_or_default();
        let engine = types::get_compose_engine(engine_id).ok_or("Invalid engine handle")?;
        engine.start(&services).await.map_err(|e| e.to_string())?;
        Ok(0u64)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_stop(engine_id: u64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let services_json = types::string_from_header(services_json_ptr).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let services: Vec<String> = serde_json::from_str(&services_json).unwrap_or_default();
        let engine = types::get_compose_engine(engine_id).ok_or("Invalid engine handle")?;
        engine.stop(&services).await.map_err(|e| e.to_string())?;
        Ok(0u64)
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_restart(engine_id: u64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let services_json = types::string_from_header(services_json_ptr).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let services: Vec<String> = serde_json::from_str(&services_json).unwrap_or_default();
        let engine = types::get_compose_engine(engine_id).ok_or("Invalid engine handle")?;
        engine.restart(&services).await.map_err(|e| e.to_string())?;
        Ok(0u64)
    });
    promise
}
