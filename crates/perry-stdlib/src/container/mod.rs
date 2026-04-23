//! Container module for Perry

pub mod backend;
pub mod types;
pub mod verification;
pub mod capability;
pub mod workload;
pub mod context;

pub use types::{
    ComposeHealthcheck, ComposeNetwork, ComposeService, ComposeSpec, ComposeVolume,
    ContainerHandle, ContainerInfo, ContainerLogs, ContainerSpec, ImageInfo, ComposeError,
};

use perry_runtime::{js_promise_new, Promise, StringHeader, js_is_truthy, js_json_parse, js_string_from_bytes, JSValue};
use backend::{detect_backend, ContainerBackend};
use std::sync::{Arc, OnceLock};
use dashmap::DashMap;
use context::ContainerContext;
use indexmap::IndexMap;

pub(crate) async fn get_global_backend_instance() -> Result<Arc<dyn ContainerBackend>, String> {
    ContainerContext::global().get_global_backend_instance().await
}

pub(crate) unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() || (ptr as usize) < 0x1000 { return None; }
    let len = (*ptr).byte_len as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    Some(String::from_utf8_lossy(bytes).to_string())
}

unsafe fn string_to_js(s: &str) -> *const StringHeader {
    let bytes = s.as_bytes();
    perry_runtime::js_string_from_bytes(bytes.as_ptr(), bytes.len() as u32)
}

fn compose_error_to_js(e: ComposeError) -> String {
    let code = match &e {
        ComposeError::NotFound(_) => 404,
        ComposeError::BackendError { code, .. } => *code,
        ComposeError::NoBackendFound { .. } => 503,
        ComposeError::BackendNotAvailable { .. } => 503,
        ComposeError::DependencyCycle { .. } => 422,
        ComposeError::ServiceStartupFailed { .. } => 500,
        ComposeError::ParseError(_) => 400,
        ComposeError::JsonError(_) => 400,
        ComposeError::IoError(_) => 500,
        ComposeError::ValidationError { .. } => 400,
        ComposeError::VerificationFailed { .. } => 403,
        ComposeError::FileNotFound { .. } => 404,
        ComposeError::ImagePullFailed { .. } => 500,
    };
    serde_json::json!({
        "message": e.to_string(),
        "code": code
    }).to_string()
}

fn backend_err_to_js(msg: String) -> String {
    serde_json::json!({
        "message": msg,
        "code": 503
    }).to_string()
}

// ============ Container API ============

#[no_mangle]
pub unsafe extern "C" fn js_container_run(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid spec JSON".into())) });
            return promise;
        }
    };
    let spec: ContainerSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js(e.to_string())) });
            return promise;
        }
    };
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        let handle = backend.run(&spec).await.map_err(compose_error_to_js)?;
        serde_json::to_string(&handle).map_err(|e| backend_err_to_js(e.to_string()))
    }, |json| {
        let str_ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        js_json_parse(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_create(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid spec JSON".into())) });
            return promise;
        }
    };
    let spec: ContainerSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js(e.to_string())) });
            return promise;
        }
    };
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        let handle = backend.create(&spec).await.map_err(compose_error_to_js)?;
        serde_json::to_string(&handle).map_err(|e| backend_err_to_js(e.to_string()))
    }, |json| {
        let str_ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        js_json_parse(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_start(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid ID".into())) });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        match backend.start(&id).await {
            Ok(()) => Ok(JSValue::undefined().bits()),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_stop(id_ptr: *const StringHeader, timeout_val: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid ID".into())) });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        let timeout = if timeout_val.is_nan() || timeout_val < 0.0 { None } else { Some(timeout_val as u32) };
        match backend.stop(&id, timeout).await {
            Ok(()) => Ok(JSValue::undefined().bits()),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_remove(id_ptr: *const StringHeader, force_val: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid ID".into())) });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        let force = js_is_truthy(force_val) != 0;
        match backend.remove(&id, force).await {
            Ok(()) => Ok(JSValue::undefined().bits()),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_list(all_val: f64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        let all = js_is_truthy(all_val) != 0;
        let list = backend.list(all).await.map_err(compose_error_to_js)?;
        serde_json::to_string(&list).map_err(|e| backend_err_to_js(e.to_string()))
    }, |json| {
        let str_ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        js_json_parse(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_inspect(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid ID".into())) });
            return promise;
        }
    };
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        let info = backend.inspect(&id).await.map_err(compose_error_to_js)?;
        serde_json::to_string(&info).map_err(|e| backend_err_to_js(e.to_string()))
    }, |json| {
        let str_ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        js_json_parse(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_logs(id_ptr: *const StringHeader, tail_val: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid ID".into())) });
            return promise;
        }
    };
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        let tail = if tail_val.is_nan() || tail_val < 0.0 { None } else { Some(tail_val as u32) };
        let logs = backend.logs(&id, tail).await.map_err(compose_error_to_js)?;
        serde_json::to_string(&logs).map_err(|e| backend_err_to_js(e.to_string()))
    }, |json| {
        let str_ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        js_json_parse(str_ptr).bits()
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
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid ID".into())) });
            return promise;
        }
    };
    let cmd_json = string_from_header(cmd_json_ptr).unwrap_or_else(|| "[]".into());
    let env_json = string_from_header(env_json_ptr).unwrap_or_else(|| "{}".into());
    let workdir = string_from_header(workdir_ptr);

    let cmd: Vec<String> = serde_json::from_str(&cmd_json).unwrap_or_default();
    let env: Option<std::collections::HashMap<String, String>> = serde_json::from_str(&env_json).ok();

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        let workdir_owned = workdir;
        let logs = backend.exec(&id, &cmd, env.as_ref(), workdir_owned.as_deref()).await.map_err(compose_error_to_js)?;
        serde_json::to_string(&logs).map_err(|e| backend_err_to_js(e.to_string()))
    }, |json| {
        let str_ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        js_json_parse(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_pull_image(ref_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let reference = match string_from_header(ref_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid reference".into())) });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        match backend.pull_image(&reference).await {
            Ok(()) => Ok(JSValue::undefined().bits()),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_list_images() -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        let list = backend.list_images().await.map_err(compose_error_to_js)?;
        serde_json::to_string(&list).map_err(|e| backend_err_to_js(e.to_string()))
    }, |json| {
        let str_ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        js_json_parse(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_remove_image(ref_ptr: *const StringHeader, force_val: f64) -> *mut Promise {
    let promise = js_promise_new();
    let reference = match string_from_header(ref_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid reference".into())) });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        let force = js_is_truthy(force_val) != 0;
        match backend.remove_image(&reference, force).await {
            Ok(()) => Ok(JSValue::undefined().bits()),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_get_backend() -> *const StringHeader {
    let ctx = ContainerContext::global();
    let name = if let Some(b) = ctx.backend.get() {
        b.backend_name()
    } else {
        match std::env::consts::OS {
            "macos" | "ios" => "apple/container".into(),
            _ => "podman".into(),
        }
    };
    string_to_js(&name)
}


#[no_mangle]
pub unsafe extern "C" fn js_container_detect_backend() -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let results = backend::probe_all_backends().await;
        serde_json::to_string(&results).map_err(|e| backend_err_to_js(e.to_string()))
    }, |json| {
        let str_ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        js_json_parse(str_ptr).bits()
    });
    promise
}

// ============ Compose API ============

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_up(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid spec JSON".into())) });
            return promise;
        }
    };

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        // Support both JSON spec and file path
        let engine = if spec_json.ends_with(".yaml") || spec_json.ends_with(".yml") {
            let config = perry_container_compose::config::ProjectConfig {
                files: vec![std::path::PathBuf::from(spec_json)],
                ..Default::default()
            };
            let project = perry_container_compose::ComposeProject::load(&config).map_err(|e| compose_error_to_js(e))?;
            let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
            perry_container_compose::ComposeEngine::new(project.spec, project.project_name, backend)
        } else {
            let spec: ComposeSpec = serde_json::from_str(&spec_json).map_err(|e| backend_err_to_js(e.to_string()))?;
            let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
            let project_name = spec.name.clone().unwrap_or_else(|| "perry-stack".into());
            perry_container_compose::ComposeEngine::new(spec, project_name, backend)
        };

        match engine.clone().up(&[], false, false, false).await {
            Ok(handle) => {
                let stack_id = types::register_compose_handle(engine);
                let full_handle = perry_container_compose::types::ComposeHandle {
                    stack_id,
                    project_name: handle.project_name,
                    services: handle.services,
                };
                serde_json::to_string(&full_handle).map_err(|e| backend_err_to_js(e.to_string()))
            }
            Err(e) => Err(compose_error_to_js(e)),
        }
    }, |json| {
        let str_ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        js_json_parse(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_down(stack_id: i64, volumes_val: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = stack_id as u64;
    let volumes = js_is_truthy(volumes_val) != 0;
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = types::COMPOSE_HANDLES.get().and_then(|m| m.get(&id).map(|e| e.clone())).ok_or_else(|| backend_err_to_js("Stack not found".into()))?;
        match engine.down(&[], volumes).await {
            Ok(()) => {
                types::COMPOSE_HANDLES.get().map(|m| m.remove(&id));
                Ok(JSValue::undefined().bits())
            }
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_ps(stack_id: i64) -> *mut Promise {
    let promise = js_promise_new();
    let id = stack_id as u64;
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let engine = types::COMPOSE_HANDLES.get().and_then(|m| m.get(&id).map(|e| e.clone())).ok_or_else(|| backend_err_to_js("Stack not found".into()))?;
        let list = engine.ps().await.map_err(compose_error_to_js)?;
        serde_json::to_string(&list).map_err(|e| backend_err_to_js(e.to_string()))
    }, |json| {
        let str_ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        js_json_parse(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_logs(stack_id: i64, service_ptr: *const StringHeader, tail_val: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = stack_id as u64;
    let service = string_from_header(service_ptr);
    let tail = if tail_val > 0.0 { Some(tail_val as u32) } else { None };

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let engine = types::COMPOSE_HANDLES.get().and_then(|m| m.get(&id).map(|e| e.clone())).ok_or_else(|| backend_err_to_js("Stack not found".into()))?;
        let logs = engine.logs(service.as_deref(), tail).await.map_err(compose_error_to_js)?;
        serde_json::to_string(&logs).map_err(|e| backend_err_to_js(e.to_string()))
    }, |json| {
        let str_ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        js_json_parse(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_exec(stack_id: i64, service_ptr: *const StringHeader, cmd_json_ptr: *const StringHeader, workdir_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = stack_id as u64;
    let service = string_from_header(service_ptr).unwrap_or_default();
    let cmd_json = string_from_header(cmd_json_ptr).unwrap_or_else(|| "[]".into());
    let workdir = string_from_header(workdir_ptr);

    let cmd: Vec<String> = serde_json::from_str(&cmd_json).unwrap_or_default();

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let engine = types::COMPOSE_HANDLES.get().and_then(|m| m.get(&id).map(|e| e.clone())).ok_or_else(|| backend_err_to_js("Stack not found".into()))?;
        let workdir_owned = workdir;
        let logs = engine.exec(&service, &cmd, None, workdir_owned.as_deref()).await.map_err(compose_error_to_js)?;
        serde_json::to_string(&logs).map_err(|e| backend_err_to_js(e.to_string()))
    }, |json| {
        let str_ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        js_json_parse(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_config(stack_id: i64) -> *mut Promise {
    let promise = js_promise_new();
    let id = stack_id as u64;
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let engine = types::COMPOSE_HANDLES.get().and_then(|m| m.get(&id).map(|e| e.clone())).ok_or_else(|| backend_err_to_js("Stack not found".into()))?;
        let yaml = engine.config().map_err(compose_error_to_js)?;
        Ok(yaml)
    }, |yaml| {
        let str_ptr = js_string_from_bytes(yaml.as_ptr(), yaml.len() as u32);
        JSValue::string_ptr(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_start(stack_id: i64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = stack_id as u64;
    let services_json = string_from_header(services_json_ptr).unwrap_or_else(|| "[]".into());
    let services: Vec<String> = serde_json::from_str(&services_json).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = types::COMPOSE_HANDLES.get().and_then(|m| m.get(&id).map(|e| e.clone())).ok_or_else(|| backend_err_to_js("Stack not found".into()))?;
        match engine.start(&services).await {
            Ok(()) => Ok(JSValue::undefined().bits()),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_stop(stack_id: i64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = stack_id as u64;
    let services_json = string_from_header(services_json_ptr).unwrap_or_else(|| "[]".into());
    let services: Vec<String> = serde_json::from_str(&services_json).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = types::COMPOSE_HANDLES.get().and_then(|m| m.get(&id).map(|e| e.clone())).ok_or_else(|| backend_err_to_js("Stack not found".into()))?;
        match engine.stop(&services).await {
            Ok(()) => Ok(JSValue::undefined().bits()),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_restart(stack_id: i64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = stack_id as u64;
    let services_json = string_from_header(services_json_ptr).unwrap_or_else(|| "[]".into());
    let services: Vec<String> = serde_json::from_str(&services_json).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = types::COMPOSE_HANDLES.get().and_then(|m| m.get(&id).map(|e| e.clone())).ok_or_else(|| backend_err_to_js("Stack not found".into()))?;
        match engine.restart(&services).await {
            Ok(()) => Ok(JSValue::undefined().bits()),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_graph(stack_id: i64) -> *const StringHeader {
    let id = stack_id as u64;
    let ctx = ContainerContext::global();
    if let Some(entry) = ctx.handles.get(&id) {
        if let context::HandleEntry::Compose(engine) = entry.value() {
            if let Ok(graph) = engine.graph() {
                if let Ok(json) = serde_json::to_string(&graph) {
                    return string_to_js(&json);
                }
            }
        }
    }
    string_to_js("{}")
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_status(stack_id: i64) -> *mut Promise {
    let promise = js_promise_new();
    let id = stack_id as u64;
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let ctx = ContainerContext::global();
        let engine = if let Some(entry) = ctx.handles.get(&id) {
            if let context::HandleEntry::Compose(engine) = entry.value() {
                Some(Arc::clone(engine))
            } else { None }
        } else { None };

        let engine = engine.ok_or_else(|| backend_err_to_js("Stack not found".into()))?;
        let status = engine.status().await.map_err(compose_error_to_js)?;
        serde_json::to_string(&status).map_err(|e| backend_err_to_js(e.to_string()))
    }, |json| {
        let str_ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        js_json_parse(str_ptr).bits()
    });
    promise
}

// ============ Workload API ============

#[no_mangle]
pub unsafe extern "C" fn js_workload_graph(name_ptr: *const StringHeader, spec_json_ptr: *const StringHeader) -> *const StringHeader {
    let name = string_from_header(name_ptr).unwrap_or_else(|| "default".into());
    let spec_json = string_from_header(spec_json_ptr).unwrap_or_else(|| "{}".into());
    let nodes: IndexMap<String, workload::WorkloadNode> = serde_json::from_str(&spec_json).unwrap_or_default();

    let mut edges = Vec::new();
    for (node_id, node) in &nodes {
        for dep_id in &node.depends_on {
            edges.push(workload::WorkloadEdge { from: node_id.clone(), to: dep_id.clone() });
        }
    }

    let graph = workload::WorkloadGraph { name, nodes, edges };
    let json = serde_json::to_string(&graph).unwrap_or_else(|_| "{}".into());
    string_to_js(&json)
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_node(name_ptr: *const StringHeader, spec_json_ptr: *const StringHeader) -> *const StringHeader {
    let name = string_from_header(name_ptr).unwrap_or_else(|| "default".into());
    let spec_json = string_from_header(spec_json_ptr).unwrap_or_else(|| "{}".into());
    let mut node: workload::WorkloadNode = serde_json::from_str(&spec_json).unwrap_or_else(|_| workload::WorkloadNode {
        id: name.clone(),
        name: name.clone(),
        image: None,
        resources: None,
        ports: vec![],
        env: std::collections::HashMap::new(),
        depends_on: vec![],
        runtime: workload::RuntimeSpec::Auto,
        policy: workload::PolicySpec { tier: workload::PolicyTier::Default, no_network: None, read_only_root: None, seccomp: None },
    });
    node.id = name.clone();
    node.name = name;
    let json = serde_json::to_string(&node).unwrap_or_else(|_| "{}".into());
    string_to_js(&json)
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_run_graph(graph_json_ptr: *const StringHeader, opts_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let graph_json = string_from_header(graph_json_ptr).unwrap_or_else(|| "{}".into());
    let opts_json = string_from_header(opts_json_ptr).unwrap_or_else(|| "{}".into());

    let graph: workload::WorkloadGraph = match serde_json::from_str(&graph_json) {
        Ok(g) => g,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js(e.to_string())) });
            return promise;
        }
    };

    let _opts: workload::RunGraphOptions = serde_json::from_str(&opts_json).unwrap_or_else(|_| workload::RunGraphOptions { strategy: None, on_failure: None });

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        let state = Arc::new(workload::WorkloadGraphState::new(graph, backend));

        state.run().await.map_err(|e| compose_error_to_js(e))?;

        let handle_id = rand::random::<u64>();
        let ctx = ContainerContext::global();
        ctx.handles.insert(handle_id, context::HandleEntry::WorkloadGraph(state));

        let handle = workload::GraphHandle { handle_id };
        serde_json::to_string(&handle).map_err(|e| backend_err_to_js(e.to_string()))
    }, |json| {
        let str_ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        js_json_parse(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_inspect_graph(graph_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let graph_json = string_from_header(graph_json_ptr).unwrap_or_else(|| "{}".into());
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let _graph: workload::WorkloadGraph = serde_json::from_str(&graph_json).map_err(|e| backend_err_to_js(e.to_string()))?;
        let status = workload::GraphStatus { nodes: std::collections::HashMap::new(), healthy: false, errors: None };
        serde_json::to_string(&status).map_err(|e| backend_err_to_js(e.to_string()))
    }, |json| {
        let str_ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        js_json_parse(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_down(handle_id: i64, _opts_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let ctx = ContainerContext::global();
        ctx.handles.remove(&id).ok_or_else(|| backend_err_to_js("Graph not found".into()))?;
        Ok(JSValue::undefined().bits())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_status(handle_id: i64) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let state = if let Some(entry) = ContainerContext::global().handles.get(&id) {
            if let context::HandleEntry::WorkloadGraph(state) = entry.value() {
                Some(Arc::clone(state))
            } else { None }
        } else { None }.ok_or_else(|| backend_err_to_js("Graph not found".into()))?;

        let status = state.status().await;
        serde_json::to_string(&status).map_err(|e| backend_err_to_js(e.to_string()))
    }, |json| {
        let str_ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        js_json_parse(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_graph(handle_id: i64) -> *const StringHeader {
    let id = handle_id as u64;
    let ctx = ContainerContext::global();
    if let Some(entry) = ctx.handles.get(&id) {
        if let context::HandleEntry::WorkloadGraph(state) = entry.value() {
            if let Ok(json) = serde_json::to_string(&state.graph) {
                return string_to_js(&json);
            }
        }
    }
    string_to_js("{}")
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_logs(handle_id: i64, node_ptr: *const StringHeader, _opts_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    let node_id = string_from_header(node_ptr).unwrap_or_default();
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let state = if let Some(entry) = ContainerContext::global().handles.get(&id) {
            if let context::HandleEntry::WorkloadGraph(state) = entry.value() {
                Some(Arc::clone(state))
            } else { None }
        } else { None }.ok_or_else(|| backend_err_to_js("Graph not found".into()))?;

        let logs = state.logs(&node_id, None).await.map_err(compose_error_to_js)?;
        serde_json::to_string(&logs).map_err(|e| backend_err_to_js(e.to_string()))
    }, |json| {
        let str_ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        js_json_parse(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_exec(handle_id: i64, node_ptr: *const StringHeader, cmd_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    let node_id = string_from_header(node_ptr).unwrap_or_default();
    let cmd_json = string_from_header(cmd_json_ptr).unwrap_or_else(|| "[]".into());
    let cmd: Vec<String> = serde_json::from_str(&cmd_json).unwrap_or_default();

    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let state = if let Some(entry) = ContainerContext::global().handles.get(&id) {
            if let context::HandleEntry::WorkloadGraph(state) = entry.value() {
                Some(Arc::clone(state))
            } else { None }
        } else { None }.ok_or_else(|| backend_err_to_js("Graph not found".into()))?;

        let logs = state.exec(&node_id, &cmd, None, None).await.map_err(compose_error_to_js)?;
        serde_json::to_string(&logs).map_err(|e| backend_err_to_js(e.to_string()))
    }, |json| {
        let str_ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        js_json_parse(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_ps(handle_id: i64) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    crate::common::spawn_for_promise_deferred(promise as *mut u8, async move {
        let state = if let Some(entry) = ContainerContext::global().handles.get(&id) {
            if let context::HandleEntry::WorkloadGraph(state) = entry.value() {
                Some(Arc::clone(state))
            } else { None }
        } else { None }.ok_or_else(|| backend_err_to_js("Graph not found".into()))?;

        let results = state.ps();
        serde_json::to_string(&results).map_err(|e| backend_err_to_js(e.to_string()))
    }, |json| {
        let str_ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        js_json_parse(str_ptr).bits()
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_module_init() {
    // We used to block_on here, but that's bad for initialization performance.
    // Detection will happen lazily on first use.
}
