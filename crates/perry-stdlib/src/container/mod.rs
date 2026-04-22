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
pub use types::{
    ComposeHandle, ComposeSpec, ContainerError, ContainerHandle,
    ContainerInfo, ContainerLogs, ContainerSpec, ImageInfo, ListOrDict,
};

use perry_runtime::{js_promise_new, Promise, StringHeader};
pub use backend::{detect_backend, ContainerBackend};
use std::sync::OnceLock;
use std::sync::Arc;
use std::collections::HashMap;
use std::ptr;

// Global backend instance - initialized once at first use
static BACKEND: OnceLock<Arc<dyn ContainerBackend>> = OnceLock::new();
static BACKEND_INIT_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Resolved environment variables for workload nodes (node_id -> { key -> value })
static WORKLOAD_RESOLVED_ENV: once_cell::sync::Lazy<dashmap::DashMap<String, HashMap<String, String>>> =
    once_cell::sync::Lazy::new(dashmap::DashMap::new);

/// Get or initialize the global backend instance
async fn get_global_backend() -> Result<&'static Arc<dyn ContainerBackend>, ContainerError> {
    if let Some(b) = BACKEND.get() {
        return Ok(b);
    }

    let _guard = BACKEND_INIT_LOCK.lock().await;

    // Double-check after acquiring lock
    if let Some(b) = BACKEND.get() {
        return Ok(b);
    }

    let b = match detect_backend().await {
        Ok(b) => Arc::new(b) as Arc<dyn ContainerBackend>,
        Err(probed) => {
            // Attempt interactive install
            match perry_container_compose::installer::BackendInstaller::run().await {
                Ok(b) => Arc::new(b) as Arc<dyn ContainerBackend>,
                Err(_) => return Err(ContainerError::NoBackendFound { probed }),
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
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
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

/// Helper for workload graph construction
/// FFI: js_workload_graph(name: *const StringHeader, spec_json: *const StringHeader) -> *const StringHeader
#[no_mangle]
pub unsafe extern "C" fn js_workload_graph(
    name_ptr: *const StringHeader,
    spec_ptr: *const StringHeader,
) -> *const StringHeader {
    let name = string_from_header(name_ptr).unwrap_or_default();
    let spec_json = string_from_header(spec_ptr).unwrap_or_default();

    let nodes: HashMap<String, workload::WorkloadNode> = serde_json::from_str(&spec_json).unwrap_or_default();
    let graph = workload::WorkloadGraph {
        name,
        nodes,
        edges: Vec::new(), // Edges are derived from depends_on in nodes
    };

    let json = serde_json::to_string(&graph).unwrap_or_default();
    string_to_js(&json)
}

/// Helper for workload node construction
/// FFI: js_workload_node(name: *const StringHeader, spec_json: *const StringHeader) -> *const StringHeader
#[no_mangle]
pub unsafe extern "C" fn js_workload_node(
    name_ptr: *const StringHeader,
    spec_ptr: *const StringHeader,
) -> *const StringHeader {
    let name = string_from_header(name_ptr).unwrap_or_default();
    let spec_json = string_from_header(spec_ptr).unwrap_or_default();

    let mut node: workload::WorkloadNode = serde_json::from_str(&spec_json).unwrap_or_else(|_| workload::WorkloadNode {
        id: name.clone(),
        name: name.clone(),
        image: None,
        resources: None,
        ports: Vec::new(),
        env: HashMap::new(),
        depends_on: Vec::new(),
        runtime: workload::RuntimeSpec::Auto,
        policy: workload::PolicySpec { tier: workload::PolicyTier::Default, no_network: None, read_only_root: None, seccomp: None },
    });
    node.name = name;

    let json = serde_json::to_string(&node).unwrap_or_default();
    string_to_js(&json)
}

/// Inspect a workload graph status without starting it
/// FFI: js_workload_inspectGraph(graph_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_workload_inspectGraph(graph_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let graph_json = string_from_header(graph_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let _graph: workload::WorkloadGraph = graph_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .ok_or("Invalid graph JSON")?;

        // Mock status for now
        let status = workload::GraphStatus {
            nodes: HashMap::new(),
            healthy: true,
            errors: None,
        };
        let json = serde_json::to_string(&status).map_err(|e| e.to_string())?;
        let h = types::register_handle(json);
        Ok(h as u64)
    });

    promise
}

/// Stop and remove a workload graph
/// FFI: js_workload_handle_down(handle_id: i64, options_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_down(
    handle_id: i64,
    options_ptr: *const StringHeader,
) -> *mut Promise {
    js_container_compose_down(handle_id, 1) // default to remove volumes for workloads
}

/// Get workload graph status
/// FFI: js_workload_handle_status(handle_id: i64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_status(handle_id: i64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = {
            let engines = perry_container_compose::compose::COMPOSE_ENGINES.lock().unwrap();
            engines.get(&(handle_id as u64)).cloned()
        };

        if let Some(engine) = engine {
            let containers = engine.ps().await.map_err(|e| e.to_string())?;
            let mut nodes = HashMap::new();
            let mut healthy = true;
            for c in containers {
                let state = if c.status.to_lowercase().contains("running") {
                    workload::NodeState::Running
                } else {
                    healthy = false;
                    workload::NodeState::Stopped
                };
                nodes.insert(c.name, state);
            }
            let status = workload::GraphStatus {
                nodes,
                healthy,
                errors: None,
            };
            let json = serde_json::to_string(&status).map_err(|e| e.to_string())?;
            let h = types::register_handle(json);
            Ok(h as u64)
        } else {
            Err("Compose engine not found".to_string())
        }
    });
    promise
}

/// Get the original graph definition from a handle
/// FFI: js_workload_handle_graph(handle_id: i64) -> *const StringHeader
#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_graph(handle_id: i64) -> *const StringHeader {
    let engine = {
        let engines = perry_container_compose::compose::COMPOSE_ENGINES.lock().unwrap();
        engines.get(&(handle_id as u64)).cloned()
    };

    if let Some(engine) = engine {
        let json = serde_json::to_string(&engine.spec).unwrap_or_default();
        string_to_js(&json)
    } else {
        string_to_js("")
    }
}

/// Get logs for a workload node
/// FFI: js_workload_handle_logs(handle_id: i64, node: *const StringHeader, options_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_logs(
    handle_id: i64,
    node_ptr: *const StringHeader,
    _options_ptr: *const StringHeader,
) -> *mut Promise {
    js_container_compose_logs(handle_id, node_ptr, -1)
}

/// Execute command in a workload node
/// FFI: js_workload_handle_exec(handle_id: i64, node: *const StringHeader, cmd_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_exec(
    handle_id: i64,
    node_ptr: *const StringHeader,
    cmd_json_ptr: *const StringHeader,
) -> *mut Promise {
    js_container_compose_exec(handle_id, node_ptr, cmd_json_ptr, ptr::null())
}

/// List nodes with detailed info
/// FFI: js_workload_handle_ps(handle_id: i64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_ps(handle_id: i64) -> *mut Promise {
    js_container_compose_ps(handle_id)
}

// ============ Workload Graph Functions ============

/// Run a workload graph
/// FFI: js_workload_runGraph(graph_json: *const StringHeader, options_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_workload_runGraph(
    graph_ptr: *const StringHeader,
    options_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();

    let graph_json = string_from_header(graph_ptr);
    let options_json = string_from_header(options_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let graph: workload::WorkloadGraph = graph_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .ok_or("Invalid graph JSON")?;

        let options: workload::RunGraphOptions = options_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(workload::RunGraphOptions { strategy: None, on_failure: None });

        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };

        // Convert WorkloadGraph to ComposeSpec for execution
        let mut spec = ComposeSpec::default();
        spec.name = Some(graph.name.clone());
        for (id, node) in &graph.nodes {
            let mut svc = perry_container_compose::types::ComposeService::default();
            svc.image = node.image.clone();
            svc.ports = Some(node.ports.iter().map(|p| perry_container_compose::types::PortSpec::Short(serde_yaml::Value::String(p.clone()))).collect());
            svc.environment = Some(perry_container_compose::types::ListOrDict::Dict(
                node.env.iter().map(|(k, v)| {
                    let val = match v {
                        workload::WorkloadEnvValue::Literal(s) => Some(serde_yaml::Value::String(s.clone())),
                        workload::WorkloadEnvValue::Ref(r) => Some(serde_yaml::Value::String(format!("REF:{}:{:?}:{:?}", r.node_id, r.projection, r.port))),
                    };
                    (k.clone(), val)
                }).collect()
            ));
            svc.depends_on = Some(perry_container_compose::types::DependsOnSpec::List(node.depends_on.clone()));
            spec.services.insert(id.clone(), svc);
        }

        let engine = Arc::new(perry_container_compose::ComposeEngine::new(spec, graph.name, backend));
        let handle = engine.up(&[], true, false, false).await.map_err(|e| e.to_string())?;

        // Resolve WorkloadRefs after containers have started
        let mut running_nodes = HashMap::new();
        let containers = engine.ps().await.map_err(|e| e.to_string())?;
        for container in containers {
            // Mapping back from container name to node ID is a bit loose here,
            // but in WorkloadGraph node_id == svc_name usually.
            running_nodes.insert(container.name.clone(), container);
        }

        for (id, node) in &graph.nodes {
            for (env_key, env_val) in &node.env {
                if let workload::WorkloadEnvValue::Ref(r) = env_val {
                    let resolved = r.resolve(&running_nodes).map_err(|e| e.to_string())?;
                    // Inject resolved value back into container
                    let container_name = running_nodes.get(id)
                        .map(|c| c.name.clone())
                        .ok_or_else(|| format!("Container for node {} not found", id))?;

                    // Store in resolved env registry for subsequent execs
                    let mut node_env = WORKLOAD_RESOLVED_ENV.entry(id.clone()).or_insert_with(HashMap::new);
                    node_env.insert(env_key.clone(), resolved.clone());

                    // Also try to inject into already running container
                    let _ = engine.backend.exec(
                        &container_name,
                        &vec![format!("export {}={}", env_key, resolved)],
                        None,
                        None
                    ).await;
                }
            }
        }

        Ok(handle.stack_id)
    });

    promise
}

/// Check if an image exists locally
/// FFI: js_container_imageExists(reference: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_imageExists(reference_ptr: *const StringHeader) -> *mut Promise {
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
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err(e.to_string()),
        };

        match backend.inspect_image(&reference).await {
            Ok(_) => Ok(1u64), // true
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
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
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
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.start(&id).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Stop a running container
/// FFI: js_container_stop(id: *const StringHeader, timeout: i64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_stop(id_ptr: *const StringHeader, timeout: i64) -> *mut Promise {
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
        let timeout_opt = if timeout < 0 { None } else { Some(timeout as u32) };
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.stop(&id, timeout_opt).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Remove a container
/// FFI: js_container_remove(id: *const StringHeader, force: i64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_remove(id_ptr: *const StringHeader, force: i64) -> *mut Promise {
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
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.remove(&id, force != 0).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// List containers
/// FFI: js_container_list(all: i64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_list(all: i64) -> *mut Promise {
    let promise = js_promise_new();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
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
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
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

/// Get the current backend name
/// FFI: js_container_getBackend() -> *const StringHeader
#[no_mangle]
pub unsafe extern "C" fn js_container_getBackend() -> *const StringHeader {
    let name = BACKEND.get().map(|b| b.backend_name()).unwrap_or("unknown");
    string_to_js(name)
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
            Err(probed) => {
                let json = serde_json::to_string(&probed).unwrap_or_default();
                Ok(json) // Resolve with probe info array on failure to find any
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

    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid container ID".to_string())
            });
            return promise;
        }
    };

    let tail_opt = if tail >= 0 { Some(tail as u32) } else { None };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
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

        let env: Option<HashMap<String, String>> = env_json
            .and_then(|s| serde_json::from_str(&s).ok());

        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
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
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
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
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
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
/// FFI: js_container_removeImage(reference: *const StringHeader, force: i64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_removeImage(reference_ptr: *const StringHeader, force: i64) -> *mut Promise {
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
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.remove_image(&reference, force != 0).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Build a container image
/// FFI: js_container_build(build_json: *const StringHeader, image_name: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_build(
    build_ptr: *const StringHeader,
    image_name_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();

    let build_json = string_from_header(build_ptr);
    let image_name = string_from_header(image_name_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let build_spec: perry_container_compose::types::ComposeServiceBuild = build_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .ok_or("Invalid build spec JSON")?;

        let name = image_name.ok_or("Invalid image name")?;

        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };

        match backend.build(&build_spec, &name).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

// ============ Compose Functions ============

/// Bring up a Compose stack
/// FFI: js_container_composeUp(spec_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_composeUp(spec_ptr: *const StringHeader) -> *mut Promise {
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
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
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
/// FFI: js_container_compose_down(handle_id: i64, volumes: i64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_down(handle_id: i64, volumes: i64) -> *mut Promise {
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

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = {
            let engines = perry_container_compose::compose::COMPOSE_ENGINES.lock().unwrap();
            engines.get(&handle.stack_id).cloned()
        };

        if let Some(engine) = engine {
            match engine.down(&[], false, volumes != 0).await {
                Ok(()) => Ok(0u64),
                Err(e) => Err::<u64, String>(e.to_string()),
            }
        } else {
            Err::<u64, String>("Compose engine not found".to_string())
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
        let engine = {
            let engines = perry_container_compose::compose::COMPOSE_ENGINES.lock().unwrap();
            engines.get(&handle.stack_id).cloned()
        };

        if let Some(engine) = engine {
            match engine.ps().await {
                Ok(containers) => {
                    let h = types::register_container_info_list(containers);
                    Ok(h as u64)
                }
                Err(e) => Err::<u64, String>(e.to_string()),
            }
        } else {
            Err::<u64, String>("Compose engine not found".to_string())
        }
    });

    promise
}

/// Get logs from compose stack
/// FFI: js_container_compose_logs(handle_id: i64, service: *const StringHeader, tail: i32) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_logs(
    handle_id: i64,
    service_ptr: *const StringHeader,
    tail: i64,
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

    let service = unsafe { string_from_header(service_ptr) };
    let tail_opt = if tail >= 0 { Some(tail as u32) } else { None };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = {
            let engines = perry_container_compose::compose::COMPOSE_ENGINES.lock().unwrap();
            engines.get(&handle.stack_id).cloned()
        };

        if let Some(engine) = engine {
            let services = service.map(|s| vec![s]).unwrap_or_default();
            match engine.logs(&services, tail_opt).await {
                Ok(logs) => {
                    let mut combined_stdout = String::new();
                    for (svc, log) in logs {
                        combined_stdout.push_str(&format!("{}: {}\n", svc, log));
                    }
                    let h = types::register_container_logs(ContainerLogs {
                        stdout: combined_stdout,
                        stderr: String::new(),
                    });
                    Ok(h as u64)
                }
                Err(e) => Err::<u64, String>(e.to_string()),
            }
        } else {
            Err::<u64, String>("Compose engine not found".to_string())
        }
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
    options_json_ptr: *const StringHeader,
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
    let options_json = unsafe { string_from_header(options_json_ptr) };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = {
            let engines = perry_container_compose::compose::COMPOSE_ENGINES.lock().unwrap();
            engines.get(&handle.stack_id).cloned()
        };

        if let Some(engine) = engine {
            let service = match service_opt {
                Some(s) => s,
                None => return Err::<u64, String>("Invalid service name".to_string()),
            };

            let cmd: Vec<String> = cmd_json
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();

            let env: Option<HashMap<String, String>> = options_json
                .as_ref()
                .and_then(|s| {
                    let v: serde_json::Value = serde_json::from_str(s).ok()?;
                    serde_json::from_value(v.get("env")?.clone()).ok()
                });

            let workdir: Option<String> = options_json
                .as_ref()
                .and_then(|s| {
                    let v: serde_json::Value = serde_json::from_str(s).ok()?;
                    v.get("workdir")?.as_str().map(|s| s.to_string())
                });

            // Merge in resolved workload environment variables if this is a workload node
            let mut final_env = env.unwrap_or_default();
            if let Some(resolved) = WORKLOAD_RESOLVED_ENV.get(&service) {
                for (k, v) in resolved.iter() {
                    final_env.insert(k.clone(), v.clone());
                }
            }

            match engine.exec(&service, &cmd, Some(&final_env), workdir.as_deref()).await {
                Ok(logs) => {
                    let h = types::register_container_logs(logs);
                    Ok(h as u64)
                }
                Err(e) => Err::<u64, String>(e.to_string()),
            }
        } else {
            Err::<u64, String>("Compose engine not found".to_string())
        }
    });

    promise
}

/// Get resolved YAML configuration
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
            let engine = {
                let engines = perry_container_compose::compose::COMPOSE_ENGINES.lock().unwrap();
                engines.get(&handle.stack_id).cloned()
            };

            if let Some(engine) = engine {
                engine.config().map_err(|e| e.to_string())
            } else {
                Err::<String, String>("Compose engine not found".to_string())
            }
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

    let services_json = unsafe { string_from_header(services_json_ptr) };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = {
            let engines = perry_container_compose::compose::COMPOSE_ENGINES.lock().unwrap();
            engines.get(&handle.stack_id).cloned()
        };

        if let Some(engine) = engine {
            let services: Vec<String> = services_json
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();

            match engine.start(&services).await {
                Ok(()) => Ok(0u64),
                Err(e) => Err::<u64, String>(e.to_string()),
            }
        } else {
            Err::<u64, String>("Compose engine not found".to_string())
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

    let services_json = unsafe { string_from_header(services_json_ptr) };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = {
            let engines = perry_container_compose::compose::COMPOSE_ENGINES.lock().unwrap();
            engines.get(&handle.stack_id).cloned()
        };

        if let Some(engine) = engine {
            let services: Vec<String> = services_json
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();

            match engine.stop(&services).await {
                Ok(()) => Ok(0u64),
                Err(e) => Err::<u64, String>(e.to_string()),
            }
        } else {
            Err::<u64, String>("Compose engine not found".to_string())
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

    let services_json = unsafe { string_from_header(services_json_ptr) };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = {
            let engines = perry_container_compose::compose::COMPOSE_ENGINES.lock().unwrap();
            engines.get(&handle.stack_id).cloned()
        };

        if let Some(engine) = engine {
            let services: Vec<String> = services_json
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();

            match engine.restart(&services).await {
                Ok(()) => Ok(0u64),
                Err(e) => Err::<u64, String>(e.to_string()),
            }
        } else {
            Err::<u64, String>("Compose engine not found".to_string())
        }
    });

    promise
}

// ============ Image Operations ============

/// Inspect an image
/// FFI: js_container_inspectImage(reference: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_inspectImage(reference_ptr: *const StringHeader) -> *mut Promise {
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
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err(e.to_string()),
        };

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
}
