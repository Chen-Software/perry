//! Container module for Perry
//!
//! Provides OCI container management with platform-adaptive backend selection.

pub mod backend;
pub mod capability;
pub mod compose;
pub mod types;
pub mod verification;

pub use self::backend::get_global_backend;

use perry_runtime::{js_promise_new, Promise, StringHeader};
use std::sync::Arc;

/// Helper to extract string from StringHeader pointer
unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() || (ptr as usize) < 0x1000 { return None; }
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
pub unsafe extern "C" fn js_container_run(spec_json: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec = match types::parse_container_spec_json(spec_json) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend();
        match backend.run(&spec).await {
            Ok(handle) => Ok(types::register_container_handle(handle)),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_inspectGraph(handle_id: u64) -> *mut Promise {
    js_workload_handle_status(handle_id)
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_graph(handle_id: u64) -> *const StringHeader {
    if let Some(engine) = types::get_workload_engine(handle_id) {
        if let Ok(json) = serde_json::to_string(&engine.graph) {
            return string_to_js(&json);
        }
    }
    string_to_js("{}")
}

#[no_mangle]
pub unsafe extern "C" fn js_container_create(spec_json: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec = match types::parse_container_spec_json(spec_json) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend();
        match backend.create(&spec).await {
            Ok(handle) => Ok(types::register_container_handle(handle)),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_start(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        get_global_backend().start(&id).await.map(|_| 0u64).map_err(|e| e.to_string())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_stop(id_ptr: *const StringHeader, timeout: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();
    let t = if timeout < 0.0 { None } else { Some(timeout as u32) };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        get_global_backend().stop(&id, t).await.map(|_| 0u64).map_err(|e| e.to_string())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_remove(id_ptr: *const StringHeader, force: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        get_global_backend().remove(&id, force != 0.0).await.map(|_| 0u64).map_err(|e| e.to_string())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_list(all: f64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match get_global_backend().list(all != 0.0).await {
            Ok(list) => Ok(types::register_container_info_list(list)),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_inspect(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match get_global_backend().inspect(&id).await {
            Ok(info) => Ok(types::register_container_info(info)),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_logs(id_ptr: *const StringHeader, tail: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();
    let t = if tail < 0.0 { None } else { Some(tail as u32) };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match get_global_backend().logs(&id, t).await {
            Ok(logs) => Ok(types::register_container_logs(logs)),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_exec(id_ptr: *const StringHeader, cmd_json: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();
    let cmd_str = string_from_header(cmd_json).unwrap_or_default();
    let cmd: Vec<String> = serde_json::from_str(&cmd_str).unwrap_or_else(|_| {
        cmd_str.split_whitespace().map(String::from).collect()
    });
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match get_global_backend().exec(&id, &cmd, None, None).await {
            Ok(logs) => Ok(types::register_container_logs(logs)),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_pullImage(image_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let image = string_from_header(image_ptr).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        get_global_backend().pull_image(&image).await.map(|_| 0u64).map_err(|e| e.to_string())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_listImages() -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match get_global_backend().list_images().await {
            Ok(list) => Ok(types::register_image_info_list(list)),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_build(spec_json: *const StringHeader, image_name_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec = match types::parse_compose_spec_json(spec_json) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };
    let image_name = string_from_header(image_name_ptr).unwrap_or_default();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend();
        // Assuming the first service's build spec for now as per simple build use case
        if let Some(service) = spec.services.values().next() {
            if let Some(build) = &service.build {
                return backend.build(&build.as_build(), &image_name).await.map(|_| 0u64).map_err(|e| e.to_string());
            }
        }
        Err("No build spec found in ComposeSpec".to_string())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_removeImage(image_ptr: *const StringHeader, force: f64) -> *mut Promise {
    let promise = js_promise_new();
    let image = string_from_header(image_ptr).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        get_global_backend().remove_image(&image, force != 0.0).await.map(|_| 0u64).map_err(|e| e.to_string())
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_getBackend() -> *const StringHeader {
    string_to_js(get_global_backend().name())
}

#[no_mangle]
pub unsafe extern "C" fn js_container_detectBackend() -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let results = perry_container_compose::detect_all_backends().await;
        match serde_json::to_string(&results) {
            Ok(json) => Ok(types::register_string(json)),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

// ============ Workload Functions ============

#[no_mangle]
pub unsafe extern "C" fn js_workload_graph(name_ptr: *const StringHeader) -> u64 {
    let name = string_from_header(name_ptr).unwrap_or_else(|| "default".to_string());
    perry_container_compose::workload::create_graph_builder(name)
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_node(graph_id: u64, name_ptr: *const StringHeader, config_json: *const StringHeader) -> u64 {
    let name = string_from_header(name_ptr).unwrap_or_default();
    let config_str = string_from_header(config_json).unwrap_or_default();
    let config: perry_container_compose::workload::WorkloadNodeConfig = serde_json::from_str(&config_str).unwrap_or_else(|_| {
        perry_container_compose::workload::WorkloadNodeConfig {
            image: "unknown".to_string(),
            ports: None, volumes: None, env: None, cmd: None, depends_on: None, runtime: None, policy: None
        }
    });
    perry_container_compose::workload::add_node_to_graph(graph_id, name, config);
    0
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_runGraph(graph_id: u64, opts_json: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let graph = match perry_container_compose::workload::get_graph_from_builder(graph_id) {
        Some(g) => g,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid graph handle".to_string()) });
            return promise;
        }
    };

    #[derive(serde::Deserialize)]
    struct RunOpts {
        strategy: perry_container_compose::workload::ExecutionStrategy,
        on_failure: perry_container_compose::workload::FailureStrategy,
    }

    let opts: RunOpts = match string_from_header(opts_json) {
        Some(s) => serde_json::from_str(&s).unwrap_or(RunOpts {
            strategy: perry_container_compose::workload::ExecutionStrategy::ParallelSafe,
            on_failure: perry_container_compose::workload::FailureStrategy::RollbackAll,
        }),
        None => RunOpts {
            strategy: perry_container_compose::workload::ExecutionStrategy::ParallelSafe,
            on_failure: perry_container_compose::workload::FailureStrategy::RollbackAll,
        },
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend();
        let adapter = Arc::new(backend::BackendAdapter { inner: backend });
        let engine = Arc::new(perry_container_compose::workload::WorkloadGraphEngine::new(graph, adapter));
        match Arc::clone(&engine).run(opts.strategy, opts.on_failure).await {
            Ok(handle_id) => Ok(types::register_workload_engine(engine, handle_id)),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_down(handle_id: u64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_workload_engine(handle_id) {
            engine.down().await.map(|_| 0u64).map_err(|e| e.to_string())
        } else {
            Err("Invalid workload handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_status(handle_id: u64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_workload_engine(handle_id) {
            match engine.status().await {
                Ok(status) => match serde_json::to_string(&status) {
                    Ok(json) => Ok(types::register_string(json)),
                    Err(e) => Err(e.to_string()),
                },
                Err(e) => Err(e.to_string()),
            }
        } else {
            Err("Invalid workload handle".to_string())
        }
    });
    promise
}

// ============ Compose Functions ============

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_up(spec_json: *const StringHeader) -> *mut Promise {
    js_container_composeUp(spec_json)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_composeUp(spec_json: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec = match types::parse_compose_spec_json(spec_json) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend();
        let adapter = Arc::new(backend::BackendAdapter { inner: backend });
        let project_name = spec.name.clone().unwrap_or_else(|| "perry-stack".to_string());
        let engine = Arc::new(perry_container_compose::ComposeEngine::new(spec, project_name, adapter));
        match Arc::clone(&engine).up(&[], true, true, false).await {
            Ok(handle) => Ok(types::register_compose_engine(engine, handle.stack_id)),
            Err(e) => Err(e.to_string()),
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_down(handle_id: u64, volumes: f64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_compose_engine(handle_id) {
            engine.down(&[], false, volumes != 0.0).await.map(|_| 0u64).map_err(|e| e.to_string())
        } else {
            Err("Invalid compose handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_logs(
    handle_id: u64,
    node_ptr: *const StringHeader,
    tail: f64,
) -> *mut Promise {
    let promise = js_promise_new();
    let node = string_from_header(node_ptr);
    let t = if tail < 0.0 { None } else { Some(tail as u32) };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_workload_engine(handle_id) {
            match engine.logs(node.as_deref(), t).await {
                Ok(logs) => {
                    let combined = logs.values().cloned().collect::<Vec<_>>().join("\n");
                    Ok(types::register_container_logs(types::ContainerLogs {
                        stdout: combined,
                        stderr: String::new(),
                    }))
                }
                Err(e) => Err(e.to_string()),
            }
        } else {
            Err("Invalid workload handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_exec(
    handle_id: u64,
    node_ptr: *const StringHeader,
    cmd_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let node = string_from_header(node_ptr).unwrap_or_default();
    let cmd_str = string_from_header(cmd_ptr).unwrap_or_default();
    let cmd: Vec<String> = serde_json::from_str(&cmd_str).unwrap_or_else(|_| {
        cmd_str.split_whitespace().map(String::from).collect()
    });
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_workload_engine(handle_id) {
            match engine.exec(&node, &cmd, None, None).await {
                Ok(res) => Ok(types::register_container_logs(types::ContainerLogs {
                    stdout: res.stdout,
                    stderr: res.stderr,
                })),
                Err(e) => Err(e.to_string()),
            }
        } else {
            Err("Invalid workload handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_ps(handle_id: u64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_workload_engine(handle_id) {
            match engine.ps().await {
                Ok(list) => Ok(types::register_container_info_list(
                    list.into_iter().map(types::ContainerInfo::from).collect(),
                )),
                Err(e) => Err(e.to_string()),
            }
        } else {
            Err("Invalid workload handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_ps(handle_id: u64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_compose_engine(handle_id) {
            match engine.ps().await {
                Ok(list) => Ok(types::register_container_info_list(
                    list.into_iter().map(types::ContainerInfo::from).collect(),
                )),
                Err(e) => Err(e.to_string()),
            }
        } else {
            Err("Invalid compose handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_logs(
    handle_id: u64,
    service_ptr: *const StringHeader,
    tail: f64,
) -> *mut Promise {
    let promise = js_promise_new();
    let service = string_from_header(service_ptr);
    let services = service
        .as_ref()
        .map(|s| vec![s.clone()])
        .unwrap_or_default();
    let t = if tail < 0.0 { None } else { Some(tail as u32) };
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_compose_engine(handle_id) {
            match engine.logs(&services, t).await {
                Ok(logs) => {
                    let combined = logs.values().cloned().collect::<Vec<_>>().join("\n");
                    Ok(types::register_container_logs(types::ContainerLogs {
                        stdout: combined,
                        stderr: String::new(),
                    }))
                }
                Err(e) => Err(e.to_string()),
            }
        } else {
            Err("Invalid compose handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_exec(
    handle_id: u64,
    service_ptr: *const StringHeader,
    cmd_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let service = string_from_header(service_ptr).unwrap_or_default();
    let cmd_str = string_from_header(cmd_ptr).unwrap_or_default();
    let cmd: Vec<String> = serde_json::from_str(&cmd_str).unwrap_or_else(|_| {
        cmd_str.split_whitespace().map(String::from).collect()
    });
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_compose_engine(handle_id) {
            match engine.exec(&service, &cmd, None, None).await {
                Ok(res) => Ok(types::register_container_logs(types::ContainerLogs {
                    stdout: res.stdout,
                    stderr: res.stderr,
                })),
                Err(e) => Err(e.to_string()),
            }
        } else {
            Err("Invalid compose handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_config(handle_id: u64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_compose_engine(handle_id) {
            match serde_json::to_string(&engine.spec) {
                Ok(json) => Ok(types::register_string(json)),
                Err(e) => Err(e.to_string()),
            }
        } else {
            Err("Invalid compose handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_start(handle_id: u64, services_json: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let services_str = string_from_header(services_json).unwrap_or_default();
    let services: Vec<String> = serde_json::from_str(&services_str).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_compose_engine(handle_id) {
            engine.start(&services).await.map(|_| 0u64).map_err(|e| e.to_string())
        } else {
            Err("Invalid compose handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_stop(handle_id: u64, services_json: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let services_str = string_from_header(services_json).unwrap_or_default();
    let services: Vec<String> = serde_json::from_str(&services_str).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_compose_engine(handle_id) {
            engine.stop(&services).await.map(|_| 0u64).map_err(|e| e.to_string())
        } else {
            Err("Invalid compose handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_restart(handle_id: u64, services_json: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let services_str = string_from_header(services_json).unwrap_or_default();
    let services: Vec<String> = serde_json::from_str(&services_str).unwrap_or_default();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_compose_engine(handle_id) {
            engine.restart(&services).await.map(|_| 0u64).map_err(|e| e.to_string())
        } else {
            Err("Invalid compose handle".to_string())
        }
    });
    promise
}

#[no_mangle]
pub extern "C" fn js_container_module_init() {
    let _ = get_global_backend();
}
