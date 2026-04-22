//! Container module for Perry

pub mod backend;
pub mod capability;
pub mod compose;
pub mod types;
pub mod verification;
mod tests;

use perry_container_compose::backend::{detect_backend, ContainerBackend};
use perry_runtime::{js_promise_new, Promise, StringHeader};
use std::sync::{Arc, OnceLock, Mutex};

static BACKEND: OnceLock<Arc<dyn ContainerBackend + Send + Sync>> = OnceLock::new();
static BACKEND_ERROR: OnceLock<String> = OnceLock::new();
static INIT_MUTEX: Mutex<()> = Mutex::new(());

pub async fn get_global_backend_instance_async() -> Result<&'static Arc<dyn ContainerBackend + Send + Sync>, String> {
    if let Some(error) = BACKEND_ERROR.get() {
        return Err(error.clone());
    }
    if let Some(backend) = BACKEND.get() {
        return Ok(backend);
    }

    let _lock = INIT_MUTEX.lock().unwrap();
    if let Some(backend) = BACKEND.get() {
        return Ok(backend);
    }

    match detect_backend().await {
        Ok(backend) => {
            let arc = Arc::from(backend);
            let _ = BACKEND.set(arc);
            Ok(BACKEND.get().unwrap())
        }
        Err(probed) => {
            let err = format!("No container backend found. Probed: {:?}", probed);
            let _ = BACKEND_ERROR.set(err.clone());
            Err(err)
        }
    }
}

pub fn get_global_backend_instance() -> Result<&'static Arc<dyn ContainerBackend + Send + Sync>, String> {
    if let Some(error) = BACKEND_ERROR.get() {
        return Err(error.clone());
    }
    if let Some(backend) = BACKEND.get() {
        return Ok(backend);
    }

    let _lock = INIT_MUTEX.lock().unwrap();
    if let Some(backend) = BACKEND.get() {
        return Ok(backend);
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio");

    runtime.block_on(async {
        match detect_backend().await {
            Ok(backend) => {
                let arc = Arc::from(backend);
                let _ = BACKEND.set(arc);
                Ok(BACKEND.get().unwrap())
            }
            Err(probed) => {
                let err = format!("No container backend found. Probed: {:?}", probed);
                let _ = BACKEND_ERROR.set(err.clone());
                Err(err)
            }
        }
    })
}

pub unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() || (ptr as usize) < 0x1000 {
        return None;
    }
    let len = (*ptr).length as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    Some(String::from_utf8_lossy(bytes).to_string())
}

pub unsafe fn string_to_js(s: &str) -> *const StringHeader {
    let bytes = s.as_bytes();
    perry_runtime::js_string_from_bytes(bytes.as_ptr(), bytes.len() as u32)
}

// ============ Container Lifecycle ============

#[no_mangle]
pub unsafe extern "C" fn js_container_run(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err(e) });
            return promise;
        }
    };

    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err("Invalid JSON".into()) });
            return promise;
        }
    };

    let spec: types::ContainerSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err(msg) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match backend.run(&spec).await {
            Ok(handle) => Ok(types::register_container_handle(handle)),
            Err(e) => Err(e.to_string()),
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_create(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err(e) });
            return promise;
        }
    };

    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err("Invalid JSON".into()) });
            return promise;
        }
    };

    let spec: types::ContainerSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err(msg) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
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
    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err(e) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match backend.start(&id).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err(e.to_string()),
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_stop(id_ptr: *const StringHeader, timeout: i32) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();
    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err(e) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match backend.stop(&id, if timeout >= 0 { Some(timeout as u32) } else { None }).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err(e.to_string()),
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_remove(id_ptr: *const StringHeader, force: i32) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();
    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err(e) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match backend.remove(&id, force != 0).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err(e.to_string()),
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
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err(e) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match backend.list(all != 0).await {
            Ok(list) => Ok(list.len() as u64),
            Err(e) => Err(e.to_string()),
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_inspect(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();
    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err(e) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match backend.inspect(&id).await {
            Ok(_info) => Ok(0u64),
            Err(e) => Err(e.to_string()),
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_logs(id_ptr: *const StringHeader, tail: i32) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();
    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err(e) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match backend.logs(&id, if tail >= 0 { Some(tail as u32) } else { None }).await {
            Ok(_logs) => Ok(0u64),
            Err(e) => Err(e.to_string()),
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_exec(
    id_ptr: *const StringHeader,
    cmd_json_ptr: *const StringHeader,
    env_json_ptr: *const StringHeader,
    workdir_ptr: *const StringHeader
) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();
    let cmd_json = string_from_header(cmd_json_ptr).unwrap_or_default();
    let env_json = string_from_header(env_json_ptr).unwrap_or_default();
    let workdir = string_from_header(workdir_ptr);

    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err(e) });
            return promise;
        }
    };

    let cmd: Vec<String> = serde_json::from_str(&cmd_json).unwrap_or_default();
    let env: Option<std::collections::HashMap<String, String>> = serde_json::from_str(&env_json).ok();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match backend.exec(&id, &cmd, env.as_ref(), workdir.as_deref()).await {
            Ok(_logs) => Ok(0u64),
            Err(e) => Err(e.to_string()),
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_pullImage(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();
    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err(e) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match backend.pull_image(&id).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err(e.to_string()),
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
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err(e) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match backend.list_images().await {
            Ok(list) => Ok(list.len() as u64),
            Err(e) => Err(e.to_string()),
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_removeImage(id_ptr: *const StringHeader, force: i32) -> *mut Promise {
    let promise = js_promise_new();
    let id = string_from_header(id_ptr).unwrap_or_default();
    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err(e) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match backend.remove_image(&id, force != 0).await {
            Ok(()) => Ok(0u64),
            Err(e) => Err(e.to_string()),
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_getBackend() -> *const StringHeader {
    if let Ok(b) = get_global_backend_instance() {
        string_to_js(b.backend_name())
    } else {
        string_to_js("none")
    }
}

// ============ Compose Functions ============

#[no_mangle]
pub unsafe extern "C" fn js_container_composeUp(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let backend = match get_global_backend_instance() {
        Ok(b) => Arc::clone(b),
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err(e) });
            return promise;
        }
    };

    let spec_json = string_from_header(spec_json_ptr).unwrap_or_default();
    let spec: types::ComposeSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err(msg) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let name = spec.name.clone().unwrap_or_else(|| "default".into());
        match compose::compose_up(spec, name, backend).await {
            Ok(id) => Ok(id),
            Err(e) => Err(e),
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_up(spec_json: *const StringHeader) -> *mut Promise {
    js_container_composeUp(spec_json)
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_down(handle_id: i64, volumes: i32) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_compose_engine(handle_id as u64) {
            engine.down(volumes != 0, false).await.map(|_| 0u64).map_err(|e| e.to_string())
        } else {
            Err("Engine not found".into())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_ps(handle_id: i64) -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_compose_engine(handle_id as u64) {
            engine.ps().await.map(|_| 0u64).map_err(|e| e.to_string())
        } else {
            Err("Engine not found".into())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_logs(handle_id: i64, service_ptr: *const StringHeader, tail: i32) -> *mut Promise {
    let promise = js_promise_new();
    let service = string_from_header(service_ptr);
    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_compose_engine(handle_id as u64) {
            engine.logs(service.as_deref(), if tail >= 0 { Some(tail as u32) } else { None })
                .await
                .map(|_| 0u64)
                .map_err(|e| e.to_string())
        } else {
            Err("Engine not found".into())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_exec(handle_id: i64, service_ptr: *const StringHeader, cmd_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let service = string_from_header(service_ptr).unwrap_or_default();
    let cmd_json = string_from_header(cmd_json_ptr).unwrap_or_default();
    let cmd: Vec<String> = serde_json::from_str(&cmd_json).unwrap_or_default();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_compose_engine(handle_id as u64) {
            engine.exec(&service, &cmd)
                .await
                .map(|_| 0u64)
                .map_err(|e| e.to_string())
        } else {
            Err("Engine not found".into())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_config(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = string_from_header(spec_json_ptr).unwrap_or_default();
    let spec: types::ComposeSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            let msg = e.to_string();
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err(msg) });
            return promise;
        }
    };

    crate::common::spawn_for_promise_deferred(
        promise as *mut u8,
        async move { serde_json::to_string_pretty(&spec).map_err(|e| e.to_string()) },
        |json| {
            let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
            perry_runtime::JSValue::string_ptr(str_ptr).bits()
        },
    );
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_start(handle_id: i64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let services_json = string_from_header(services_json_ptr).unwrap_or_default();
    let services: Vec<String> = serde_json::from_str(&services_json).unwrap_or_default();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_compose_engine(handle_id as u64) {
            engine.start(&services)
                .await
                .map(|_| 0u64)
                .map_err(|e| e.to_string())
        } else {
            Err("Engine not found".into())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_stop(handle_id: i64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let services_json = string_from_header(services_json_ptr).unwrap_or_default();
    let services: Vec<String> = serde_json::from_str(&services_json).unwrap_or_default();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_compose_engine(handle_id as u64) {
            engine.stop(&services)
                .await
                .map(|_| 0u64)
                .map_err(|e| e.to_string())
        } else {
            Err("Engine not found".into())
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_restart(handle_id: i64, services_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let services_json = string_from_header(services_json_ptr).unwrap_or_default();
    let services: Vec<String> = serde_json::from_str(&services_json).unwrap_or_default();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Some(engine) = types::get_compose_engine(handle_id as u64) {
            engine.restart(&services)
                .await
                .map(|_| 0u64)
                .map_err(|e| e.to_string())
        } else {
            Err("Engine not found".into())
        }
    });
    promise
}

#[no_mangle]
pub extern "C" fn js_container_module_init() {
    let _ = get_global_backend_instance();
}
