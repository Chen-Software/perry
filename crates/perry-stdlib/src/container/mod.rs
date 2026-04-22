//! Container module for Perry

pub mod backend;
pub mod compose;
pub mod types;
pub mod verification;
pub mod capability;

pub use types::{
    ComposeHealthcheck, ComposeNetwork, ComposeService, ComposeSpec, ComposeVolume,
    ContainerHandle, ContainerInfo, ContainerLogs, ContainerSpec, ImageInfo, ComposeError,
};

use perry_runtime::{js_promise_new, Promise, StringHeader};
use backend::{detect_backend, ContainerBackend};
use std::collections::HashMap;
use indexmap::IndexMap;
use std::sync::{Arc, OnceLock};
use dashmap::DashMap;
use crate::container::types::*;

static BACKEND: OnceLock<Arc<dyn ContainerBackend>> = OnceLock::new();
static COMPOSE_ENGINES: OnceLock<DashMap<u64, Arc<perry_container_compose::ComposeEngine>>> = OnceLock::new();
static WORKLOAD_HANDLES: OnceLock<DashMap<u64, Arc<perry_container_compose::WorkloadGraphEngine>>> = OnceLock::new();

async fn get_global_backend_instance() -> Result<Arc<dyn ContainerBackend>, String> {
    if let Some(b) = BACKEND.get() {
        return Ok(Arc::clone(b));
    }
    match detect_backend().await {
        Ok(b) => {
            let arc_b: Arc<dyn ContainerBackend> = Arc::from(b);
            let _ = BACKEND.set(Arc::clone(&arc_b));
            Ok(arc_b)
        }
        Err(probed) => {
            // Interactive installer fallback per Requirement 20
            let installer = perry_container_compose::installer::BackendInstaller::new();
            match installer.run().await {
                Ok(b) => {
                    let arc_b: Arc<dyn ContainerBackend> = Arc::from(b);
                    let _ = BACKEND.set(Arc::clone(&arc_b));
                    Ok(arc_b)
                }
                Err(_) => Err(format!("No container backend found. Probed: {:?}", probed)),
            }
        }
    }
}

unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
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
        ComposeError::ImagePullFailed { .. } => 502,
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
    let spec_json = if spec_json_ptr.is_null() {
        crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid spec JSON".into())) });
        return promise;
    } else {
        match string_from_header(spec_json_ptr) {
            Some(s) => s,
            None => {
                crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid spec JSON".into())) });
                return promise;
            }
        }
    };
    let spec: ContainerSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js(e.to_string())) });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        match backend.run(&spec).await {
            Ok(handle) => Ok(types::register_container_handle(handle)),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_inspectGraph(graph_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let graph_json = match string_from_header(graph_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid graph JSON".into())) });
            return promise;
        }
    };
    let graph: WorkloadGraph = match serde_json::from_str(&graph_json) {
        Ok(g) => g,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js(e.to_string())) });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let mut nodes = HashMap::new();
        for id in graph.nodes.keys() {
            nodes.insert(id.clone(), NodeState::Unknown);
        }
        let status = GraphStatus { nodes, healthy: false, errors: None };
        Ok(types::register_graph_status(status))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_status(handle_id: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = WORKLOAD_HANDLES.get().and_then(|m| m.get(&id).map(|e| Arc::clone(e.value()))).ok_or_else(|| backend_err_to_js("Graph handle not found".into()))?;
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        let spec = ComposeSpec::default();
        let compose_engine = perry_container_compose::ComposeEngine::new(spec, engine.project_name.clone(), backend);
        let containers = compose_engine.ps().await.map_err(|e| compose_error_to_js(e))?;

        let mut nodes = HashMap::new();
        let mut healthy = true;
        for (node_id, node) in &engine.graph.nodes {
            let state = if containers.iter().any(|c| c.name == node.name && (c.status.contains("Up") || c.status.contains("running"))) {
                NodeState::Running
            } else {
                healthy = false;
                NodeState::Stopped
            };
            nodes.insert(node_id.clone(), state);
        }
        Ok(types::register_graph_status(GraphStatus { nodes, healthy, errors: None }))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_graph(handle_id: f64) -> *const StringHeader {
    let id = handle_id as u64;
    let graph_json = WORKLOAD_HANDLES.get()
        .and_then(|m| m.get(&id).map(|e| serde_json::to_string(&e.graph).unwrap_or_default()))
        .unwrap_or_else(|| "{}".into());
    string_to_js(&graph_json)
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_logs(handle_id: f64, node_ptr: *const StringHeader, opts_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    let node_id = string_from_header(node_ptr);
    let opts_json = if opts_json_ptr.is_null() { "{}".into() } else { string_from_header(opts_json_ptr).unwrap_or_else(|| "{}".into()) };
    let tail = if opts_json.contains("tail") { Some(100u32) } else { None };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = WORKLOAD_HANDLES.get().and_then(|m| m.get(&id).map(|e| Arc::clone(e.value()))).ok_or_else(|| backend_err_to_js("Graph handle not found".into()))?;
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;

        let container_name = if let Some(ref nid) = node_id {
            engine.graph.nodes.get(nid).map(|n| n.name.clone()).ok_or_else(|| backend_err_to_js("Node not found".into()))?
        } else {
            return Err(backend_err_to_js("Node required for logs".into()));
        };

        match backend.logs(&container_name, tail).await {
            Ok(logs) => Ok(types::register_container_logs(logs)),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_exec(handle_id: f64, node_ptr: *const StringHeader, cmd_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    let node_id = match string_from_header(node_ptr) {
        Some(s) => s,
        None => {
             crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid node name".into())) });
             return promise;
        }
    };
    let cmd_json = if cmd_json_ptr.is_null() { "[]".into() } else { string_from_header(cmd_json_ptr).unwrap_or_else(|| "[]".into()) };
    let cmd: Vec<String> = serde_json::from_str(&cmd_json).unwrap_or_default();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = WORKLOAD_HANDLES.get().and_then(|m| m.get(&id).map(|e| Arc::clone(e.value()))).ok_or_else(|| backend_err_to_js("Graph handle not found".into()))?;
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;

        let container_name = engine.graph.nodes.get(&node_id).map(|n| n.name.clone()).ok_or_else(|| backend_err_to_js("Node not found".into()))?;

        match backend.exec(&container_name, &cmd, None, None).await {
            Ok(logs) => Ok(types::register_container_logs(logs)),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_ps(handle_id: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = WORKLOAD_HANDLES.get().and_then(|m| m.get(&id).map(|e| Arc::clone(e.value()))).ok_or_else(|| backend_err_to_js("Graph handle not found".into()))?;
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        let spec = ComposeSpec::default();
        let compose_engine = perry_container_compose::ComposeEngine::new(spec, engine.project_name.clone(), backend);
        let containers = compose_engine.ps().await.map_err(|e| compose_error_to_js(e))?;

        let mut nodes = Vec::new();
        for (node_id, node) in &engine.graph.nodes {
            let container = containers.iter().find(|c| c.name == node.name);
            let state = if let Some(c) = container {
                if c.status.contains("Up") || c.status.contains("running") { NodeState::Running } else { NodeState::Stopped }
            } else {
                NodeState::Unknown
            };
            nodes.push(NodeInfo {
                node_id: node_id.clone(),
                name: node.name.clone(),
                container_id: container.map(|c| c.id.clone()),
                state,
                image: node.image.clone(),
            });
        }
        Ok(types::register_node_info_list(nodes))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_down(handle_id: f64, opts_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id as u64;
    let opts_json = if opts_json_ptr.is_null() { "{}".into() } else { string_from_header(opts_json_ptr).unwrap_or_else(|| "{}".into()) };
    let remove_volumes = opts_json.contains("volumes\":true");

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = WORKLOAD_HANDLES.get().and_then(|m| m.get(&id).map(|e| Arc::clone(e.value()))).ok_or_else(|| backend_err_to_js("Graph handle not found".into()))?;
        // We reuse the ComposeEngine's down logic by proxy
        // Since we don't have direct access to engine.down, let's assume we can lookup by project name
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        let spec = ComposeSpec::default(); // Dummy for now
        let compose_engine = perry_container_compose::ComposeEngine::new(spec, engine.project_name.clone(), backend);
        match compose_engine.down(remove_volumes).await {
            Ok(()) => {
                WORKLOAD_HANDLES.get().map(|m| m.remove(&id));
                Ok(0u64)
            }
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_create(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = if spec_json_ptr.is_null() {
        crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid spec JSON".into())) });
        return promise;
    } else {
        match string_from_header(spec_json_ptr) {
            Some(s) => s,
            None => {
                crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid spec JSON".into())) });
                return promise;
            }
        }
    };
    let spec: ContainerSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js(e.to_string())) });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        match backend.create(&spec).await {
            Ok(handle) => Ok(types::register_container_handle(handle)),
            Err(e) => Err(compose_error_to_js(e)),
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
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid ID".into())) });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        match backend.start(&id).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_stop(id_ptr: *const StringHeader, timeout: f64) -> *mut Promise {
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
        match backend.stop(&id, if timeout >= 0.0 { Some(timeout as u32) } else { None }).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_remove(id_ptr: *const StringHeader, force: f64) -> *mut Promise {
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
        match backend.remove(&id, force != 0.0).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_list(all: f64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        match backend.list(all != 0.0).await {
            Ok(list) => Ok(types::register_container_info_list(list)),
            Err(e) => Err(compose_error_to_js(e)),
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
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid ID".into())) });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        match backend.inspect(&id).await {
            Ok(info) => Ok(types::register_container_info(info)),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_logs(id_ptr: *const StringHeader, tail: f64) -> *mut Promise {
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
        match backend.logs(&id, if tail >= 0.0 { Some(tail as u32) } else { None }).await {
            Ok(logs) => Ok(types::register_container_logs(logs)),
            Err(e) => Err(compose_error_to_js(e)),
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
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid ID".into())) });
            return promise;
        }
    };
    let cmd_json = string_from_header(cmd_json_ptr).unwrap_or_else(|| "[]".into());
    let env_json = string_from_header(env_json_ptr).unwrap_or_else(|| "{}".into());
    let workdir = string_from_header(workdir_ptr);

    let cmd: Vec<String> = serde_json::from_str(&cmd_json).unwrap_or_default();
    let env: Option<std::collections::HashMap<String, String>> = serde_json::from_str(&env_json).ok();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        match backend.exec(&id, &cmd, env.as_ref(), workdir.as_deref()).await {
            Ok(logs) => Ok(types::register_container_logs(logs)),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_pullImage(ref_ptr: *const StringHeader) -> *mut Promise {
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
            Ok(()) => Ok(0u64),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_listImages() -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        match backend.list_images().await {
            Ok(list) => Ok(types::register_image_info_list(list)),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_removeImage(ref_ptr: *const StringHeader, force: f64) -> *mut Promise {
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
        match backend.remove_image(&reference, force != 0.0).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_getBackend() -> *const StringHeader {
    let name = if let Some(b) = BACKEND.get() {
        b.backend_name()
    } else {
        match std::env::consts::OS {
            "macos" | "ios" => "apple/container".into(),
            _ => "podman".into(),
        }
    };
    string_to_js(&name)
}

// ============ Compose API ============

#[no_mangle]
pub unsafe extern "C" fn js_container_composeUp(spec_json_ptr: *const StringHeader) -> *mut Promise {
    js_container_compose_up(spec_json_ptr)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_up(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    if spec_json_ptr.is_null() {
        crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid spec JSON".into())) });
        return promise;
    }
    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid spec JSON".into())) });
            return promise;
        }
    };
    let spec: ComposeSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js(e.to_string())) });
            return promise;
        }
    };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        let project_name = spec.name.clone().unwrap_or_else(|| "perry-stack".into());
        let engine = Arc::new(perry_container_compose::ComposeEngine::new(spec, project_name, backend));
        match engine.up(false).await {
            Ok(handle) => {
                let id = handle.stack_id;
                COMPOSE_ENGINES.get_or_init(DashMap::new).insert(id, Arc::clone(&engine));
                Ok(types::register_compose_handle(handle))
            }
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_down(stack_id: f64, volumes: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = stack_id as u64;
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = COMPOSE_ENGINES.get().and_then(|m| m.get(&id).map(|e| Arc::clone(e.value()))).ok_or_else(|| backend_err_to_js("Stack not found".into()))?;
        match engine.down(volumes != 0.0).await {
            Ok(()) => {
                COMPOSE_ENGINES.get().map(|m| m.remove(&id));
                Ok(0u64)
            }
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_ps(stack_id: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = stack_id as u64;
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = COMPOSE_ENGINES.get().and_then(|m| m.get(&id).map(|e| Arc::clone(e.value()))).ok_or_else(|| backend_err_to_js("Stack not found".into()))?;
        match engine.ps().await {
            Ok(list) => Ok(types::register_container_info_list(list)),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_logs(stack_id: f64, service_ptr: *const StringHeader, tail: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = stack_id as u64;
    let service = if service_ptr.is_null() { None } else { string_from_header(service_ptr) };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = COMPOSE_ENGINES.get().and_then(|m| m.get(&id).map(|e| Arc::clone(e.value()))).ok_or_else(|| backend_err_to_js("Stack not found".into()))?;
        match engine.logs(service.as_deref(), if tail >= 0.0 { Some(tail as u32) } else { None }).await {
            Ok(logs) => Ok(types::register_container_logs(logs)),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_exec(stack_id: f64, service_ptr: *const StringHeader, cmd_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = stack_id as u64;
    if service_ptr.is_null() {
        crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid service name".into())) });
        return promise;
    }
    let service = match string_from_header(service_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid service name".into())) });
            return promise;
        }
    };
    let cmd_json = if cmd_json_ptr.is_null() { "[]".into() } else { string_from_header(cmd_json_ptr).unwrap_or_else(|| "[]".into()) };
    let cmd: Vec<String> = serde_json::from_str(&cmd_json).unwrap_or_default();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = COMPOSE_ENGINES.get().and_then(|m| m.get(&id).map(|e| Arc::clone(e.value()))).ok_or_else(|| backend_err_to_js("Stack not found".into()))?;
        match engine.exec(&service, &cmd).await {
            Ok(logs) => Ok(types::register_container_logs(logs)),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_config(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    if spec_json_ptr.is_null() {
         crate::common::spawn_for_promise(promise as *mut u8, async move { Ok(types::register_container_logs(ContainerLogs { stdout: "{}".into(), stderr: "".into() })) });
         return promise;
    }
    let spec_json = string_from_header(spec_json_ptr).unwrap_or_else(|| "{}".into());
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        Ok(types::register_container_logs(ContainerLogs { stdout: spec_json, stderr: "".into() }))
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_start(stack_id: f64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = stack_id as u64;
    let services_json = if services_json_ptr.is_null() { "[]".into() } else { string_from_header(services_json_ptr).unwrap_or_else(|| "[]".into()) };
    let services: Vec<String> = serde_json::from_str(&services_json).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = COMPOSE_ENGINES.get().and_then(|m| m.get(&id).map(|e| Arc::clone(e.value()))).ok_or_else(|| backend_err_to_js("Stack not found".into()))?;
        match engine.start(&services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_stop(stack_id: f64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = stack_id as u64;
    let services_json = if services_json_ptr.is_null() { "[]".into() } else { string_from_header(services_json_ptr).unwrap_or_else(|| "[]".into()) };
    let services: Vec<String> = serde_json::from_str(&services_json).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = COMPOSE_ENGINES.get().and_then(|m| m.get(&id).map(|e| Arc::clone(e.value()))).ok_or_else(|| backend_err_to_js("Stack not found".into()))?;
        match engine.stop(&services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_restart(stack_id: f64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = stack_id as u64;
    let services_json = if services_json_ptr.is_null() { "[]".into() } else { string_from_header(services_json_ptr).unwrap_or_else(|| "[]".into()) };
    let services: Vec<String> = serde_json::from_str(&services_json).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = COMPOSE_ENGINES.get().and_then(|m| m.get(&id).map(|e| Arc::clone(e.value()))).ok_or_else(|| backend_err_to_js("Stack not found".into()))?;
        match engine.restart(&services).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

// ============ Workload API ============

#[no_mangle]
pub unsafe extern "C" fn js_workload_graph(name_ptr: *const StringHeader, nodes_json_ptr: *const StringHeader, edges_json_ptr: *const StringHeader) -> *const StringHeader {
    let name = string_from_header(name_ptr).unwrap_or_else(|| "unnamed".into());
    let nodes_json = string_from_header(nodes_json_ptr).unwrap_or_else(|| "{}".into());
    let edges_json = string_from_header(edges_json_ptr).unwrap_or_else(|| "[]".into());

    let nodes: IndexMap<String, WorkloadNode> = serde_json::from_str(&nodes_json).unwrap_or_default();
    let edges: Vec<WorkloadEdge> = serde_json::from_str(&edges_json).unwrap_or_default();

    let graph = WorkloadGraph { name, nodes, edges };
    string_to_js(&serde_json::to_string(&graph).unwrap_or_default())
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_node(name_ptr: *const StringHeader, spec_json_ptr: *const StringHeader) -> *const StringHeader {
    let _name = string_from_header(name_ptr).unwrap_or_else(|| "unnamed".into());
    let spec_json = string_from_header(spec_json_ptr).unwrap_or_else(|| "{}".into());
    // This is a bit of a placeholder, normally node() builder does more in TS
    string_to_js(&spec_json)
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_runGraph(graph_json_ptr: *const StringHeader, opts_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let graph_json = match string_from_header(graph_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js("Invalid graph JSON".into())) });
            return promise;
        }
    };
    let opts_json = if opts_json_ptr.is_null() { "{}".into() } else { string_from_header(opts_json_ptr).unwrap_or_else(|| "{}".into()) };

    let graph: WorkloadGraph = match serde_json::from_str(&graph_json) {
        Ok(g) => g,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(backend_err_to_js(e.to_string())) });
            return promise;
        }
    };
    let opts: RunGraphOptions = serde_json::from_str(&opts_json).unwrap_or_else(|_| RunGraphOptions { strategy: None, on_failure: None });

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(backend_err_to_js)?;
        let project_name = graph.name.clone();
        let engine = Arc::new(perry_container_compose::WorkloadGraphEngine::new(graph, project_name, backend));
        match engine.run(&opts).await {
            Ok(handle) => {
                let id = handle.stack_id;
                WORKLOAD_HANDLES.get_or_init(DashMap::new).insert(id, Arc::clone(&engine));
                Ok(types::register_compose_handle(handle))
            }
            Err(e) => Err(compose_error_to_js(e)),
        }
    });
    promise
}

#[no_mangle]
pub extern "C" fn js_container_module_init() {
    tokio::spawn(async {
        let _ = get_global_backend_instance().await;
    });
}
