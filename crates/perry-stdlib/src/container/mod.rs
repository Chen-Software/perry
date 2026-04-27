#[cfg(feature = "container")]
pub mod types;
#[cfg(feature = "container")]
pub mod context;
#[cfg(feature = "container")]
pub mod verification;
#[cfg(feature = "container")]
pub mod capability;
#[cfg(feature = "container")]
pub mod workload;

use crate::common::{StringHeader, spawn_for_promise_deferred};
use crate::core::Promise;
use perry_container_compose::backend::get_global_backend_instance;
use perry_container_compose::types::*;
use perry_container_compose::error::ComposeError;
use perry_container_compose::compose::ComposeEngine;
use std::sync::Arc;
use std::collections::HashMap;

#[no_mangle]
pub unsafe extern "C" fn js_container_module_init() {
    // Force backend detection
    tokio::spawn(async {
        let _ = get_global_backend_instance().await;
    });
}

#[no_mangle]
pub unsafe extern "C" fn js_container_run(spec_ptr: *const StringHeader) -> *mut Promise {
    let spec_json = match StringHeader::read(spec_ptr) {
        Some(s) => s,
        None => return Promise::reject("Invalid spec pointer"),
    };

    spawn_for_promise_deferred(async move {
        let spec: ContainerSpec = serde_json::from_str(&spec_json).map_err(|e| e.to_string())?;
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let id = backend.run(&spec).await.map_err(|e| e.to_string())?;
        Ok(serde_json::to_string(&id).unwrap())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_create(spec_ptr: *const StringHeader) -> *mut Promise {
    let spec_json = match StringHeader::read(spec_ptr) {
        Some(s) => s,
        None => return Promise::reject("Invalid spec pointer"),
    };

    spawn_for_promise_deferred(async move {
        let spec: ContainerSpec = serde_json::from_str(&spec_json).map_err(|e| e.to_string())?;
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let id = backend.create(&spec).await.map_err(|e| e.to_string())?;
        Ok(serde_json::to_string(&id).unwrap())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_start(id_ptr: *const StringHeader) -> *mut Promise {
    let id = match StringHeader::read(id_ptr) {
        Some(s) => s,
        None => return Promise::reject("Invalid ID pointer"),
    };

    spawn_for_promise_deferred(async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        backend.start(&id).await.map_err(|e| e.to_string())?;
        Ok("null".to_string())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_stop(id_ptr: *const StringHeader, timeout: f64) -> *mut Promise {
    let id = match StringHeader::read(id_ptr) {
        Some(s) => s,
        None => return Promise::reject("Invalid ID pointer"),
    };

    spawn_for_promise_deferred(async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let t = if timeout >= 0.0 { Some(timeout as u32) } else { None };
        backend.stop(&id, t).await.map_err(|e| e.to_string())?;
        Ok("null".to_string())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_remove(id_ptr: *const StringHeader, force: f64) -> *mut Promise {
    let id = match StringHeader::read(id_ptr) {
        Some(s) => s,
        None => return Promise::reject("Invalid ID pointer"),
    };

    spawn_for_promise_deferred(async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        backend.remove(&id, force > 0.0).await.map_err(|e| e.to_string())?;
        Ok("null".to_string())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_list(all: f64) -> *mut Promise {
    spawn_for_promise_deferred(async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let list = backend.list(all > 0.0).await.map_err(|e| e.to_string())?;
        Ok(serde_json::to_string(&list).unwrap())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_inspect(id_ptr: *const StringHeader) -> *mut Promise {
    let id = match StringHeader::read(id_ptr) {
        Some(s) => s,
        None => return Promise::reject("Invalid ID pointer"),
    };

    spawn_for_promise_deferred(async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let info = backend.inspect(&id).await.map_err(|e| e.to_string())?;
        Ok(serde_json::to_string(&info).unwrap())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_logs(id_ptr: *const StringHeader, tail: f64) -> *mut Promise {
    let id = match StringHeader::read(id_ptr) {
        Some(s) => s,
        None => return Promise::reject("Invalid ID pointer"),
    };

    spawn_for_promise_deferred(async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let t = if tail >= 0.0 { Some(tail as u32) } else { None };
        let logs = backend.logs(&id, t).await.map_err(|e| e.to_string())?;
        Ok(serde_json::to_string(&logs).unwrap())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_exec(
    id_ptr: *const StringHeader,
    cmd_ptr: *const StringHeader,
    env_ptr: *const StringHeader,
    workdir_ptr: *const StringHeader,
) -> *mut Promise {
    let id = match StringHeader::read(id_ptr) {
        Some(s) => s,
        None => return Promise::reject("Invalid ID pointer"),
    };
    let cmd_json = match StringHeader::read(cmd_ptr) {
        Some(s) => s,
        None => return Promise::reject("Invalid cmd pointer"),
    };
    let env_json = StringHeader::read(env_ptr);
    let workdir = StringHeader::read(workdir_ptr);

    spawn_for_promise_deferred(async move {
        let cmd: Vec<String> = serde_json::from_str(&cmd_json).map_err(|e| e.to_string())?;
        let env: Option<HashMap<String, String>> = env_json.and_then(|j| serde_json::from_str(&j).ok());
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let logs = backend.exec(&id, &cmd, env.as_ref(), workdir.as_deref()).await.map_err(|e| e.to_string())?;
        Ok(serde_json::to_string(&logs).unwrap())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_pull_image(ref_ptr: *const StringHeader) -> *mut Promise {
    let reference = match StringHeader::read(ref_ptr) {
        Some(s) => s,
        None => return Promise::reject("Invalid ref pointer"),
    };

    spawn_for_promise_deferred(async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        backend.pull_image(&reference).await.map_err(|e| e.to_string())?;
        Ok("null".to_string())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_list_images() -> *mut Promise {
    spawn_for_promise_deferred(async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let list = backend.list_images().await.map_err(|e| e.to_string())?;
        Ok(serde_json::to_string(&list).unwrap())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_remove_image(ref_ptr: *const StringHeader, force: f64) -> *mut Promise {
    let reference = match StringHeader::read(ref_ptr) {
        Some(s) => s,
        None => return Promise::reject("Invalid ref pointer"),
    };

    spawn_for_promise_deferred(async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        backend.remove_image(&reference, force > 0.0).await.map_err(|e| e.to_string())?;
        Ok("null".to_string())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_get_backend() -> *const StringHeader {
    // Note: This needs to be sync but backend detection is async.
    // In production we'd use the cached value or a placeholder.
    "none".into()
}

#[no_mangle]
pub unsafe extern "C" fn js_container_detect_backend() -> *mut Promise {
    spawn_for_promise_deferred(async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        Ok(backend.backend_name().to_string())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_up(spec_ptr: *const StringHeader) -> *mut Promise {
    let spec_json = match StringHeader::read(spec_ptr) {
        Some(s) => s,
        None => return Promise::reject("Invalid spec pointer"),
    };

    spawn_for_promise_deferred(async move {
        let spec: ComposeSpec = serde_json::from_str(&spec_json).map_err(|e| e.to_string())?;
        let engine = ComposeEngine::new(spec).await.map_err(|e| e.to_string())?;
        engine.up().await.map_err(|e| e.to_string())?;
        // For now, return a fixed handle ID. Real implementation would use a registry.
        Ok("1".to_string())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_down(handle: f64, volumes: f64) -> *mut Promise {
    spawn_for_promise_deferred(async move {
        // Mock implementation
        let _ = (handle, volumes);
        Ok("null".to_string())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_ps(handle: f64) -> *mut Promise {
    spawn_for_promise_deferred(async move {
        let _ = handle;
        Ok("[]".to_string())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_logs(handle: f64, opts_ptr: *const StringHeader) -> *mut Promise {
    let _opts = StringHeader::read(opts_ptr);
    spawn_for_promise_deferred(async move {
        let _ = handle;
        Ok(r#"{"stdout":"","stderr":""}"#.to_string())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_exec(
    handle: f64,
    service_ptr: *const StringHeader,
    cmd_ptr: *const StringHeader,
    opts_ptr: *const StringHeader,
) -> *mut Promise {
    let _service = StringHeader::read(service_ptr);
    let _cmd = StringHeader::read(cmd_ptr);
    let _opts = StringHeader::read(opts_ptr);
    spawn_for_promise_deferred(async move {
        let _ = handle;
        Ok(r#"{"stdout":"","stderr":""}"#.to_string())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_run_graph(graph_ptr: *const StringHeader, opts_ptr: *const StringHeader) -> *mut Promise {
    let _graph = StringHeader::read(graph_ptr);
    let _opts = StringHeader::read(opts_ptr);
    spawn_for_promise_deferred(async move {
        Ok("1".to_string())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_inspect_graph(graph_ptr: *const StringHeader) -> *mut Promise {
    let _graph = StringHeader::read(graph_ptr);
    spawn_for_promise_deferred(async move {
        Ok(r#"{"nodes":{},"healthy":true}"#.to_string())
    })
}
