pub mod backend;
pub mod capability;
pub mod compose;
pub mod types;
pub mod verification;

pub use types::*;

use crate::common::async_bridge::{queue_promise_resolution, spawn};
use crate::common::handle::{get_handle, register_handle};
use perry_container_compose::backend::{detect_backend, ContainerBackend};
use perry_container_compose::compose::ComposeEngine;
use perry_container_compose::types::{
    ComposeHandle as InternalComposeHandle, ComposeSpec as InternalComposeSpec,
    ContainerSpec as InternalContainerSpec,
};
use perry_runtime::{js_promise_new, js_string_from_bytes, JSValue, Promise, StringHeader};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

static BACKEND: OnceLock<Arc<dyn ContainerBackend>> = OnceLock::new();

pub async fn get_global_backend() -> Arc<dyn ContainerBackend> {
    if let Some(b) = BACKEND.get() {
        return Arc::clone(b);
    }
    let b = Arc::from(
        detect_backend()
            .await
            .expect("Failed to detect container backend"),
    );
    let _ = BACKEND.set(Arc::clone(&b));
    b
}

pub fn get_global_backend_sync() -> Option<Arc<dyn ContainerBackend>> {
    BACKEND.get().cloned()
}

unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() || (ptr as usize) < 0x1000 {
        return None;
    }
    let len = (*ptr).byte_len as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    std::str::from_utf8(bytes).ok().map(|s| s.to_string())
}

#[no_mangle]
pub unsafe extern "C" fn js_container_getBackend() -> *mut StringHeader {
    if let Some(backend) = get_global_backend_sync() {
        let name = backend.backend_name();
        return js_string_from_bytes(name.as_ptr(), name.len() as u32);
    }
    std::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn js_container_run(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let promise_ptr = promise as usize;
    let json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            queue_promise_resolution(promise_ptr, false, JSValue::undefined().bits());
            return promise;
        }
    };

    spawn(async move {
        let spec: InternalContainerSpec = match serde_json::from_str(&json) {
            Ok(s) => s,
            Err(e) => {
                let err_msg = format!("Invalid ContainerSpec: {}", e);
                let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                queue_promise_resolution(promise_ptr, false, JSValue::string_ptr(err_str).bits());
                return;
            }
        };

        let backend = get_global_backend().await;
        match backend.run(&spec).await {
            Ok(handle) => {
                let h = register_handle(handle);
                queue_promise_resolution(promise_ptr, true, (h as f64).to_bits());
            }
            Err(e) => {
                let err_msg = e.to_string();
                let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                queue_promise_resolution(promise_ptr, false, JSValue::string_ptr(err_str).bits());
            }
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_create(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let promise_ptr = promise as usize;
    let json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            queue_promise_resolution(promise_ptr, false, JSValue::undefined().bits());
            return promise;
        }
    };

    spawn(async move {
        let spec: InternalContainerSpec = match serde_json::from_str(&json) {
            Ok(s) => s,
            Err(e) => {
                let err_msg = format!("Invalid ContainerSpec: {}", e);
                let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                queue_promise_resolution(promise_ptr, false, JSValue::string_ptr(err_str).bits());
                return;
            }
        };

        let backend = get_global_backend().await;
        match backend.create(&spec).await {
            Ok(handle) => {
                let h = register_handle(handle);
                queue_promise_resolution(promise_ptr, true, (h as f64).to_bits());
            }
            Err(e) => {
                let err_msg = e.to_string();
                let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                queue_promise_resolution(promise_ptr, false, JSValue::string_ptr(err_str).bits());
            }
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_start(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let promise_ptr = promise as usize;
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            queue_promise_resolution(promise_ptr, false, JSValue::undefined().bits());
            return promise;
        }
    };
    spawn(async move {
        let backend = get_global_backend().await;
        match backend.start(&id).await {
            Ok(_) => queue_promise_resolution(promise_ptr, true, JSValue::undefined().bits()),
            Err(e) => {
                let err_str =
                    js_string_from_bytes(e.to_string().as_ptr(), e.to_string().len() as u32);
                queue_promise_resolution(promise_ptr, false, JSValue::string_ptr(err_str).bits());
            }
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
    let promise_ptr = promise as usize;
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            queue_promise_resolution(promise_ptr, false, JSValue::undefined().bits());
            return promise;
        }
    };
    let t = if timeout >= 0 {
        Some(timeout as u32)
    } else {
        None
    };
    spawn(async move {
        let backend = get_global_backend().await;
        match backend.stop(&id, t).await {
            Ok(_) => queue_promise_resolution(promise_ptr, true, JSValue::undefined().bits()),
            Err(e) => {
                let err_str =
                    js_string_from_bytes(e.to_string().as_ptr(), e.to_string().len() as u32);
                queue_promise_resolution(promise_ptr, false, JSValue::string_ptr(err_str).bits());
            }
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_remove(
    id_ptr: *const StringHeader,
    force: i32,
) -> *mut Promise {
    let promise = js_promise_new();
    let promise_ptr = promise as usize;
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            queue_promise_resolution(promise_ptr, false, JSValue::undefined().bits());
            return promise;
        }
    };
    let f = force != 0;
    spawn(async move {
        let backend = get_global_backend().await;
        match backend.remove(&id, f).await {
            Ok(_) => queue_promise_resolution(promise_ptr, true, JSValue::undefined().bits()),
            Err(e) => {
                let err_str =
                    js_string_from_bytes(e.to_string().as_ptr(), e.to_string().len() as u32);
                queue_promise_resolution(promise_ptr, false, JSValue::string_ptr(err_str).bits());
            }
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_list(all: i32) -> *mut Promise {
    let promise = js_promise_new();
    let promise_ptr = promise as usize;
    let a = all != 0;
    spawn(async move {
        let backend = get_global_backend().await;
        match backend.list(a).await {
            Ok(infos) => {
                let json = serde_json::to_string(&infos).unwrap_or_default();
                let s = js_string_from_bytes(json.as_ptr(), json.len() as u32);
                queue_promise_resolution(promise_ptr, true, JSValue::string_ptr(s).bits());
            }
            Err(e) => {
                let err_str =
                    js_string_from_bytes(e.to_string().as_ptr(), e.to_string().len() as u32);
                queue_promise_resolution(promise_ptr, false, JSValue::string_ptr(err_str).bits());
            }
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_inspect(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let promise_ptr = promise as usize;
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            queue_promise_resolution(promise_ptr, false, JSValue::undefined().bits());
            return promise;
        }
    };
    spawn(async move {
        let backend = get_global_backend().await;
        match backend.inspect(&id).await {
            Ok(info) => {
                let json = serde_json::to_string(&info).unwrap_or_default();
                let s = js_string_from_bytes(json.as_ptr(), json.len() as u32);
                queue_promise_resolution(promise_ptr, true, JSValue::string_ptr(s).bits());
            }
            Err(e) => {
                let err_str =
                    js_string_from_bytes(e.to_string().as_ptr(), e.to_string().len() as u32);
                queue_promise_resolution(promise_ptr, false, JSValue::string_ptr(err_str).bits());
            }
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_logs(id_ptr: *const StringHeader, tail: i32) -> *mut Promise {
    let promise = js_promise_new();
    let promise_ptr = promise as usize;
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            queue_promise_resolution(promise_ptr, false, JSValue::undefined().bits());
            return promise;
        }
    };
    let t = if tail >= 0 { Some(tail as u32) } else { None };
    spawn(async move {
        let backend = get_global_backend().await;
        match backend.logs(&id, t).await {
            Ok(logs) => {
                let json = serde_json::to_string(&logs).unwrap_or_default();
                let s = js_string_from_bytes(json.as_ptr(), json.len() as u32);
                queue_promise_resolution(promise_ptr, true, JSValue::string_ptr(s).bits());
            }
            Err(e) => {
                let err_str =
                    js_string_from_bytes(e.to_string().as_ptr(), e.to_string().len() as u32);
                queue_promise_resolution(promise_ptr, false, JSValue::string_ptr(err_str).bits());
            }
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
    let promise_ptr = promise as usize;
    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            queue_promise_resolution(promise_ptr, false, JSValue::undefined().bits());
            return promise;
        }
    };
    let cmd_json = match string_from_header(cmd_json_ptr) {
        Some(s) => s,
        None => "[]".to_string(),
    };
    let env_json = string_from_header(env_json_ptr);
    let workdir = string_from_header(workdir_ptr);

    spawn(async move {
        let cmd: Vec<String> = serde_json::from_str(&cmd_json).unwrap_or_default();
        let env: Option<HashMap<String, String>> =
            env_json.and_then(|s| serde_json::from_str(&s).ok());
        let backend = get_global_backend().await;
        match backend.exec(&id, &cmd, env.as_ref(), workdir.as_deref()).await {
            Ok(logs) => {
                let json = serde_json::to_string(&logs).unwrap_or_default();
                let s = js_string_from_bytes(json.as_ptr(), json.len() as u32);
                queue_promise_resolution(promise_ptr, true, JSValue::string_ptr(s).bits());
            }
            Err(e) => {
                let err_str =
                    js_string_from_bytes(e.to_string().as_ptr(), e.to_string().len() as u32);
                queue_promise_resolution(promise_ptr, false, JSValue::string_ptr(err_str).bits());
            }
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_pullImage(ref_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let promise_ptr = promise as usize;
    let reference = match string_from_header(ref_ptr) {
        Some(s) => s,
        None => {
            queue_promise_resolution(promise_ptr, false, JSValue::undefined().bits());
            return promise;
        }
    };
    spawn(async move {
        let backend = get_global_backend().await;
        match backend.pull_image(&reference).await {
            Ok(_) => queue_promise_resolution(promise_ptr, true, JSValue::undefined().bits()),
            Err(e) => {
                let err_str =
                    js_string_from_bytes(e.to_string().as_ptr(), e.to_string().len() as u32);
                queue_promise_resolution(promise_ptr, false, JSValue::string_ptr(err_str).bits());
            }
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_listImages() -> *mut Promise {
    let promise = js_promise_new();
    let promise_ptr = promise as usize;
    spawn(async move {
        let backend = get_global_backend().await;
        match backend.list_images().await {
            Ok(images) => {
                let json = serde_json::to_string(&images).unwrap_or_default();
                let s = js_string_from_bytes(json.as_ptr(), json.len() as u32);
                queue_promise_resolution(promise_ptr, true, JSValue::string_ptr(s).bits());
            }
            Err(e) => {
                let err_str =
                    js_string_from_bytes(e.to_string().as_ptr(), e.to_string().len() as u32);
                queue_promise_resolution(promise_ptr, false, JSValue::string_ptr(err_str).bits());
            }
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_removeImage(
    ref_ptr: *const StringHeader,
    force: i32,
) -> *mut Promise {
    let promise = js_promise_new();
    let promise_ptr = promise as usize;
    let reference = match string_from_header(ref_ptr) {
        Some(s) => s,
        None => {
            queue_promise_resolution(promise_ptr, false, JSValue::undefined().bits());
            return promise;
        }
    };
    let f = force != 0;
    spawn(async move {
        let backend = get_global_backend().await;
        match backend.remove_image(&reference, f).await {
            Ok(_) => queue_promise_resolution(promise_ptr, true, JSValue::undefined().bits()),
            Err(e) => {
                let err_str =
                    js_string_from_bytes(e.to_string().as_ptr(), e.to_string().len() as u32);
                queue_promise_resolution(promise_ptr, false, JSValue::string_ptr(err_str).bits());
            }
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_composeUp(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let promise_ptr = promise as usize;
    let json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            queue_promise_resolution(promise_ptr, false, JSValue::undefined().bits());
            return promise;
        }
    };

    spawn(async move {
        let spec: InternalComposeSpec = match serde_json::from_str(&json) {
            Ok(s) => s,
            Err(e) => {
                let err_msg = format!("Invalid ComposeSpec: {}", e);
                let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                queue_promise_resolution(promise_ptr, false, JSValue::string_ptr(err_str).bits());
                return;
            }
        };

        match compose::compose_up(spec).await {
            Ok((engine, handle)) => {
                let h = register_handle(engine);
                queue_promise_resolution(promise_ptr, true, (h as f64).to_bits());
            }
            Err(e) => {
                let err_msg = e.to_string();
                let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                queue_promise_resolution(promise_ptr, false, JSValue::string_ptr(err_str).bits());
            }
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_up(spec_json_ptr: *const StringHeader) -> *mut Promise {
    js_container_composeUp(spec_json_ptr)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_down(handle_id: f64, volumes: i32) -> *mut Promise {
    let promise = js_promise_new();
    let promise_ptr = promise as usize;
    let h = handle_id as i64;
    let v = volumes != 0;
    spawn(async move {
        if let Some(engine) = get_handle::<ComposeEngine>(h) {
            match engine.down(&[], v, false).await {
                Ok(_) => queue_promise_resolution(promise_ptr, true, JSValue::undefined().bits()),
                Err(e) => {
                    let err_str =
                        js_string_from_bytes(e.to_string().as_ptr(), e.to_string().len() as u32);
                    queue_promise_resolution(promise_ptr, false, JSValue::string_ptr(err_str).bits());
                }
            }
        } else {
            queue_promise_resolution(promise_ptr, false, JSValue::undefined().bits());
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_ps(handle_id: f64) -> *mut Promise {
    let promise = js_promise_new();
    let promise_ptr = promise as usize;
    let h = handle_id as i64;
    spawn(async move {
        if let Some(engine) = get_handle::<ComposeEngine>(h) {
            match engine.ps().await {
                Ok(infos) => {
                    let json = serde_json::to_string(&infos).unwrap_or_default();
                    let s = js_string_from_bytes(json.as_ptr(), json.len() as u32);
                    queue_promise_resolution(promise_ptr, true, JSValue::string_ptr(s).bits());
                }
                Err(e) => {
                    let err_str =
                        js_string_from_bytes(e.to_string().as_ptr(), e.to_string().len() as u32);
                    queue_promise_resolution(promise_ptr, false, JSValue::string_ptr(err_str).bits());
                }
            }
        } else {
            queue_promise_resolution(promise_ptr, false, JSValue::undefined().bits());
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_logs(
    handle_id: f64,
    service_ptr: *const StringHeader,
    tail: i32,
) -> *mut Promise {
    let promise = js_promise_new();
    let promise_ptr = promise as usize;
    let h = handle_id as i64;
    let service = string_from_header(service_ptr);
    let t = if tail >= 0 { Some(tail as u32) } else { None };
    spawn(async move {
        let services = match service {
            Some(s) => vec![s],
            None => vec![],
        };
        if let Some(engine) = get_handle::<ComposeEngine>(h) {
            match engine.logs(&services, t).await {
                Ok(logs) => {
                    let json = serde_json::to_string(&logs).unwrap_or_default();
                    let s = js_string_from_bytes(json.as_ptr(), json.len() as u32);
                    queue_promise_resolution(promise_ptr, true, JSValue::string_ptr(s).bits());
                }
                Err(e) => {
                    let err_str =
                        js_string_from_bytes(e.to_string().as_ptr(), e.to_string().len() as u32);
                    queue_promise_resolution(promise_ptr, false, JSValue::string_ptr(err_str).bits());
                }
            }
        } else {
            queue_promise_resolution(promise_ptr, false, JSValue::undefined().bits());
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_exec(
    handle_id: f64,
    service_ptr: *const StringHeader,
    cmd_json_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let promise_ptr = promise as usize;
    let h = handle_id as i64;
    let service = match string_from_header(service_ptr) {
        Some(s) => s,
        None => {
            queue_promise_resolution(promise_ptr, false, JSValue::undefined().bits());
            return promise;
        }
    };
    let cmd_json = match string_from_header(cmd_json_ptr) {
        Some(s) => s,
        None => "[]".to_string(),
    };
    spawn(async move {
        let cmd: Vec<String> = serde_json::from_str(&cmd_json).unwrap_or_default();
        if let Some(engine) = get_handle::<ComposeEngine>(h) {
            match engine.exec(&service, &cmd).await {
                Ok(logs) => {
                    let json = serde_json::to_string(&logs).unwrap_or_default();
                    let s = js_string_from_bytes(json.as_ptr(), json.len() as u32);
                    queue_promise_resolution(promise_ptr, true, JSValue::string_ptr(s).bits());
                }
                Err(e) => {
                    let err_str =
                        js_string_from_bytes(e.to_string().as_ptr(), e.to_string().len() as u32);
                    queue_promise_resolution(promise_ptr, false, JSValue::string_ptr(err_str).bits());
                }
            }
        } else {
            queue_promise_resolution(promise_ptr, false, JSValue::undefined().bits());
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_config(
    spec_json_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let promise_ptr = promise as usize;
    let json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            queue_promise_resolution(promise_ptr, false, JSValue::undefined().bits());
            return promise;
        }
    };
    spawn(async move {
        let spec: InternalComposeSpec = match serde_json::from_str(&json) {
            Ok(s) => s,
            Err(e) => {
                let err_msg = format!("Invalid ComposeSpec: {}", e);
                let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                queue_promise_resolution(promise_ptr, false, JSValue::string_ptr(err_str).bits());
                return;
            }
        };
        let yaml = match spec.to_yaml() {
            Ok(y) => y,
            Err(e) => {
                let err_str =
                    js_string_from_bytes(e.to_string().as_ptr(), e.to_string().len() as u32);
                queue_promise_resolution(promise_ptr, false, JSValue::string_ptr(err_str).bits());
                return;
            }
        };
        let s = js_string_from_bytes(yaml.as_ptr(), yaml.len() as u32);
        queue_promise_resolution(promise_ptr, true, JSValue::string_ptr(s).bits());
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_start(
    handle_id: f64,
    services_json_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let promise_ptr = promise as usize;
    let h = handle_id as i64;
    let services_json = string_from_header(services_json_ptr);
    spawn(async move {
        let services: Vec<String> = services_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        if let Some(engine) = get_handle::<ComposeEngine>(h) {
            match engine.start(&services).await {
                Ok(_) => queue_promise_resolution(promise_ptr, true, JSValue::undefined().bits()),
                Err(e) => {
                    let err_str =
                        js_string_from_bytes(e.to_string().as_ptr(), e.to_string().len() as u32);
                    queue_promise_resolution(promise_ptr, false, JSValue::string_ptr(err_str).bits());
                }
            }
        } else {
            queue_promise_resolution(promise_ptr, false, JSValue::undefined().bits());
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_stop(
    handle_id: f64,
    services_json_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let promise_ptr = promise as usize;
    let h = handle_id as i64;
    let services_json = string_from_header(services_json_ptr);
    spawn(async move {
        let services: Vec<String> = services_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        if let Some(engine) = get_handle::<ComposeEngine>(h) {
            match engine.stop(&services).await {
                Ok(_) => queue_promise_resolution(promise_ptr, true, JSValue::undefined().bits()),
                Err(e) => {
                    let err_str =
                        js_string_from_bytes(e.to_string().as_ptr(), e.to_string().len() as u32);
                    queue_promise_resolution(promise_ptr, false, JSValue::string_ptr(err_str).bits());
                }
            }
        } else {
            queue_promise_resolution(promise_ptr, false, JSValue::undefined().bits());
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_restart(
    handle_id: f64,
    services_json_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let promise_ptr = promise as usize;
    let h = handle_id as i64;
    let services_json = string_from_header(services_json_ptr);
    spawn(async move {
        let services: Vec<String> = services_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        if let Some(engine) = get_handle::<ComposeEngine>(h) {
            match engine.restart(&services).await {
                Ok(_) => queue_promise_resolution(promise_ptr, true, JSValue::undefined().bits()),
                Err(e) => {
                    let err_str =
                        js_string_from_bytes(e.to_string().as_ptr(), e.to_string().len() as u32);
                    queue_promise_resolution(promise_ptr, false, JSValue::string_ptr(err_str).bits());
                }
            }
        } else {
            queue_promise_resolution(promise_ptr, false, JSValue::undefined().bits());
        }
    });
    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_detectBackend() -> *mut Promise {
    let promise = js_promise_new();
    let promise_ptr = promise as usize;
    spawn(async move {
        match detect_backend().await {
            Ok(backend) => {
                let info = serde_json::json!([{
                    "name": backend.backend_name(),
                    "available": true,
                    "reason": "detected",
                    "version": "unknown"
                }]);
                let json = info.to_string();
                let s = js_string_from_bytes(json.as_ptr(), json.len() as u32);
                queue_promise_resolution(promise_ptr, true, JSValue::string_ptr(s).bits());
            }
            Err(e) => {
                if let perry_container_compose::error::ComposeError::NoBackendFound { probed } = e {
                    let json = serde_json::to_string(&probed).unwrap_or_default();
                    let s = js_string_from_bytes(json.as_ptr(), json.len() as u32);
                    queue_promise_resolution(promise_ptr, true, JSValue::string_ptr(s).bits());
                } else {
                    let err_str =
                        js_string_from_bytes(e.to_string().as_ptr(), e.to_string().len() as u32);
                    queue_promise_resolution(promise_ptr, false, JSValue::string_ptr(err_str).bits());
                }
            }
        }
    });
    promise
}
