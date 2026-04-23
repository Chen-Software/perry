use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;
use perry_container_compose as compose;
use crate::common::{async_bridge, handle};
use perry_runtime::{JSValue, StringHeader, js_string_from_bytes};
use once_cell::sync::Lazy;
use std::collections::HashMap;

static BACKEND: OnceLock<Arc<dyn compose::ContainerBackend>> = OnceLock::new();
static BACKEND_INIT_MUTEX: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

struct ContainerHandle(String);
struct ComposeHandle(Arc<Mutex<compose::ComposeEngine>>);

async fn get_global_backend_instance() -> Result<Arc<dyn compose::ContainerBackend>, compose::ComposeError> {
    if let Some(backend) = BACKEND.get() {
        return Ok(backend.clone());
    }

    let _guard = BACKEND_INIT_MUTEX.lock().await;
    if let Some(backend) = BACKEND.get() {
        return Ok(backend.clone());
    }

    let backend = compose::detect_backend().await?;
    let _ = BACKEND.set(backend.clone());
    Ok(backend)
}

unsafe fn string_from_header(header: *const StringHeader) -> String {
    if header.is_null() || (header as usize) < 0x1000 {
        return "".to_string();
    }
    let h = &*header;
    let bytes = std::slice::from_raw_parts(header.add(1) as *const u8, h.byte_len as usize);
    String::from_utf8_lossy(bytes).to_string()
}

#[no_mangle]
pub extern "C" fn js_container_getBackend() -> *const StringHeader {
    let name = BACKEND.get().map(|b| b.backend_name()).unwrap_or("unknown");
    js_string_from_bytes(name.as_ptr(), name.len() as u32)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_detectBackend(promise_ptr: *mut u8) {
    async_bridge::spawn_for_promise_deferred(promise_ptr, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        Ok(backend.backend_name().to_string())
    }, |name| {
        let ptr = js_string_from_bytes(name.as_ptr(), name.len() as u32);
        JSValue::string_ptr(ptr).bits()
    });
}

#[no_mangle]
pub unsafe extern "C" fn js_container_run(spec_header: *const StringHeader, promise_ptr: *mut u8) {
    let spec_json = string_from_header(spec_header);
    async_bridge::spawn_for_promise_deferred(promise_ptr, async move {
        let spec: compose::ContainerSpec = serde_json::from_str(&spec_json).map_err(|e| e.to_string())?;
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let id = backend.run(&spec).await.map_err(|e| e.to_string())?;
        let h = handle::register_handle(ContainerHandle(id));
        Ok(h as f64)
    }, |h| JSValue::number(h).bits());
}

#[no_mangle]
pub unsafe extern "C" fn js_container_create(spec_header: *const StringHeader, promise_ptr: *mut u8) {
    let spec_json = string_from_header(spec_header);
    async_bridge::spawn_for_promise_deferred(promise_ptr, async move {
        let spec: compose::ContainerSpec = serde_json::from_str(&spec_json).map_err(|e| e.to_string())?;
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let id = backend.create(&spec).await.map_err(|e| e.to_string())?;
        let h = handle::register_handle(ContainerHandle(id));
        Ok(h as f64)
    }, |h| JSValue::number(h).bits());
}

#[no_mangle]
pub unsafe extern "C" fn js_container_start(id_header: *const StringHeader, promise_ptr: *mut u8) {
    let id = string_from_header(id_header);
    async_bridge::spawn_for_promise_deferred(promise_ptr, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        backend.start(&id).await.map_err(|e| e.to_string())?;
        Ok(())
    }, |_| JSValue::undefined().bits());
}

#[no_mangle]
pub unsafe extern "C" fn js_container_stop(id_header: *const StringHeader, opts_header: *const StringHeader, promise_ptr: *mut u8) {
    let id = string_from_header(id_header);
    let opts_json = string_from_header(opts_header);
    async_bridge::spawn_for_promise_deferred(promise_ptr, async move {
        let timeout = if !opts_json.is_empty() {
            let v: serde_json::Value = serde_json::from_str(&opts_json).ok().unwrap_or(serde_json::Value::Null);
            v["timeout"].as_u64().map(|t| t as u32)
        } else { None };
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        backend.stop(&id, timeout).await.map_err(|e| e.to_string())?;
        Ok(())
    }, |_| JSValue::undefined().bits());
}

#[no_mangle]
pub unsafe extern "C" fn js_container_remove(id_header: *const StringHeader, opts_header: *const StringHeader, promise_ptr: *mut u8) {
    let id = string_from_header(id_header);
    let opts_json = string_from_header(opts_header);
    async_bridge::spawn_for_promise_deferred(promise_ptr, async move {
        let force = if !opts_json.is_empty() {
            let v: serde_json::Value = serde_json::from_str(&opts_json).ok().unwrap_or(serde_json::Value::Null);
            v["force"].as_bool().unwrap_or(false)
        } else { false };
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        backend.remove(&id, force).await.map_err(|e| e.to_string())?;
        Ok(())
    }, |_| JSValue::undefined().bits());
}

#[no_mangle]
pub unsafe extern "C" fn js_container_list(opts_header: *const StringHeader, promise_ptr: *mut u8) {
    let opts_json = string_from_header(opts_header);
    async_bridge::spawn_for_promise_deferred(promise_ptr, async move {
        let all = if !opts_json.is_empty() {
            let v: serde_json::Value = serde_json::from_str(&opts_json).ok().unwrap_or(serde_json::Value::Null);
            v["all"].as_bool().unwrap_or(false)
        } else { false };
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let containers = backend.list(all).await.map_err(|e| e.to_string())?;
        Ok(serde_json::to_string(&containers).unwrap_or_default())
    }, |json| {
        let ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(ptr).bits()
    });
}

#[no_mangle]
pub unsafe extern "C" fn js_container_inspect(id_header: *const StringHeader, promise_ptr: *mut u8) {
    let id = string_from_header(id_header);
    async_bridge::spawn_for_promise_deferred(promise_ptr, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let info = backend.inspect(&id).await.map_err(|e| e.to_string())?;
        Ok(serde_json::to_string(&info).unwrap_or_default())
    }, |json| {
        let ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(ptr).bits()
    });
}

#[no_mangle]
pub unsafe extern "C" fn js_container_logs(id_header: *const StringHeader, opts_header: *const StringHeader, promise_ptr: *mut u8) {
    let id = string_from_header(id_header);
    let opts_json = string_from_header(opts_header);
    async_bridge::spawn_for_promise_deferred(promise_ptr, async move {
        let tail = if !opts_json.is_empty() {
            let v: serde_json::Value = serde_json::from_str(&opts_json).ok().unwrap_or(serde_json::Value::Null);
            v["tail"].as_u64().map(|t| t as u32)
        } else { None };
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let logs = backend.logs(&id, tail).await.map_err(|e| e.to_string())?;
        Ok(serde_json::to_string(&logs).unwrap_or_default())
    }, |json| {
        let ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(ptr).bits()
    });
}

#[no_mangle]
pub unsafe extern "C" fn js_container_exec(id_header: *const StringHeader, cmd_header: *const StringHeader, env_header: *const StringHeader, workdir_header: *const StringHeader, promise_ptr: *mut u8) {
    let id = string_from_header(id_header);
    let cmd_json = string_from_header(cmd_header);
    let env_json = string_from_header(env_header);
    let workdir = if workdir_header.is_null() { None } else { Some(string_from_header(workdir_header)) };

    async_bridge::spawn_for_promise_deferred(promise_ptr, async move {
        let cmd: Vec<String> = serde_json::from_str(&cmd_json).map_err(|e| e.to_string())?;
        let env: Option<HashMap<String, String>> = if !env_json.is_empty() {
            serde_json::from_str(&env_json).ok()
        } else { None };
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let logs = backend.exec(&id, &cmd, env.as_ref(), workdir.as_deref()).await.map_err(|e| e.to_string())?;
        Ok(serde_json::to_string(&logs).unwrap_or_default())
    }, |json| {
        let ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(ptr).bits()
    });
}

#[no_mangle]
pub unsafe extern "C" fn js_container_pullImage(ref_header: *const StringHeader, promise_ptr: *mut u8) {
    let reference = string_from_header(ref_header);
    async_bridge::spawn_for_promise_deferred(promise_ptr, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        backend.pull_image(&reference).await.map_err(|e| e.to_string())?;
        Ok(())
    }, |_| JSValue::undefined().bits());
}

#[no_mangle]
pub unsafe extern "C" fn js_container_listImages(promise_ptr: *mut u8) {
    async_bridge::spawn_for_promise_deferred(promise_ptr, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let images = backend.list_images().await.map_err(|e| e.to_string())?;
        Ok(serde_json::to_string(&images).unwrap_or_default())
    }, |json| {
        let ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(ptr).bits()
    });
}

#[no_mangle]
pub unsafe extern "C" fn js_container_removeImage(ref_header: *const StringHeader, force: i32, promise_ptr: *mut u8) {
    let reference = string_from_header(ref_header);
    async_bridge::spawn_for_promise_deferred(promise_ptr, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        backend.remove_image(&reference, force != 0).await.map_err(|e| e.to_string())?;
        Ok(())
    }, |_| JSValue::undefined().bits());
}

#[no_mangle]
pub unsafe extern "C" fn js_container_composeUp(spec_header: *const StringHeader, promise_ptr: *mut u8) {
    let input = string_from_header(spec_header);
    async_bridge::spawn_for_promise_deferred(promise_ptr, async move {
        let spec = compose::ComposeProject::load(&input).map_err(|e| e.to_string())?;
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let mut engine = compose::ComposeEngine::new(backend, spec, None);
        engine.up().await.map_err(|e| e.to_string())?;
        let h = handle::register_handle(ComposeHandle(Arc::new(Mutex::new(engine))));
        Ok(h as f64)
    }, |h| JSValue::number(h).bits());
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_down(handle: f64, volumes: f64, promise_ptr: *mut u8) {
    let h = handle as i64;
    let vols = volumes != 0.0;
    async_bridge::spawn_for_promise_deferred(promise_ptr, async move {
        if let Some(ch) = handle::take_handle::<ComposeHandle>(h) {
            let mut engine = ch.0.lock().await;
            engine.down(vols).await.map_err(|e| e.to_string())?;
            Ok(())
        } else {
            Err("Invalid compose handle".to_string())
        }
    }, |_| JSValue::undefined().bits());
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_ps(handle: f64, promise_ptr: *mut u8) {
    let h = handle as i64;
    async_bridge::spawn_for_promise_deferred(promise_ptr, async move {
        if let Some(ch) = handle::get_handle::<ComposeHandle>(h) {
            let engine = ch.0.lock().await;
            let info = engine.ps().await.map_err(|e| e.to_string())?;
            Ok(serde_json::to_string(&info).unwrap_or_default())
        } else {
            Err("Invalid compose handle".to_string())
        }
    }, |json| {
        let ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(ptr).bits()
    });
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_logs(handle: f64, opts_header: *const StringHeader, promise_ptr: *mut u8) {
    let h = handle as i64;
    let opts_json = string_from_header(opts_header);
    async_bridge::spawn_for_promise_deferred(promise_ptr, async move {
        let (service, tail) = if !opts_json.is_empty() {
            let v: serde_json::Value = serde_json::from_str(&opts_json).ok().unwrap_or(serde_json::Value::Null);
            (v["service"].as_str().map(|s| s.to_string()), v["tail"].as_u64().map(|t| t as u32))
        } else { (None, None) };
        if let Some(ch) = handle::get_handle::<ComposeHandle>(h) {
            let engine = ch.0.lock().await;
            let logs = engine.logs(service, tail).await.map_err(|e| e.to_string())?;
            Ok(serde_json::to_string(&logs).unwrap_or_default())
        } else { Err("Invalid compose handle".to_string()) }
    }, |json| {
        let ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(ptr).bits()
    });
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_exec(handle: f64, service_header: *const StringHeader, cmd_header: *const StringHeader, env_header: *const StringHeader, promise_ptr: *mut u8) {
    let h = handle as i64;
    let service = string_from_header(service_header);
    let cmd_json = string_from_header(cmd_header);
    let env_json = string_from_header(env_header);
    async_bridge::spawn_for_promise_deferred(promise_ptr, async move {
        let cmd: Vec<String> = serde_json::from_str(&cmd_json).map_err(|e| e.to_string())?;
        let env: Option<HashMap<String, String>> = if !env_json.is_empty() {
            serde_json::from_str(&env_json).ok()
        } else { None };
        if let Some(ch) = handle::get_handle::<ComposeHandle>(h) {
            let engine = ch.0.lock().await;
            let logs = engine.exec(service, &cmd, env.as_ref(), None).await.map_err(|e| e.to_string())?;
            Ok(serde_json::to_string(&logs).unwrap_or_default())
        } else { Err("Invalid compose handle".to_string()) }
    }, |json| {
        let ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
        JSValue::string_ptr(ptr).bits()
    });
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_config(handle: f64, promise_ptr: *mut u8) {
    let h = handle as i64;
    async_bridge::spawn_for_promise_deferred(promise_ptr, async move {
        if let Some(ch) = handle::get_handle::<ComposeHandle>(h) {
            let engine = ch.0.lock().await;
            Ok(engine.config().map_err(|e| e.to_string())?)
        } else {
            Err("Invalid compose handle".to_string())
        }
    }, |yaml| {
        let ptr = js_string_from_bytes(yaml.as_ptr(), yaml.len() as u32);
        JSValue::string_ptr(ptr).bits()
    });
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_start(handle: f64, services_header: *const StringHeader, promise_ptr: *mut u8) {
    let h = handle as i64;
    let services_json = string_from_header(services_header);
    async_bridge::spawn_for_promise_deferred(promise_ptr, async move {
        let services: Option<Vec<String>> = if !services_json.is_empty() {
            serde_json::from_str(&services_json).ok()
        } else { None };
        if let Some(ch) = handle::get_handle::<ComposeHandle>(h) {
            let engine = ch.0.lock().await;
            engine.start(services).await.map_err(|e| e.to_string())?;
            Ok(())
        } else { Err("Invalid compose handle".to_string()) }
    }, |_| JSValue::undefined().bits());
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_stop(handle: f64, services_header: *const StringHeader, promise_ptr: *mut u8) {
    let h = handle as i64;
    let services_json = string_from_header(services_header);
    async_bridge::spawn_for_promise_deferred(promise_ptr, async move {
        let services: Option<Vec<String>> = if !services_json.is_empty() {
            serde_json::from_str(&services_json).ok()
        } else { None };
        if let Some(ch) = handle::get_handle::<ComposeHandle>(h) {
            let engine = ch.0.lock().await;
            engine.stop(services).await.map_err(|e| e.to_string())?;
            Ok(())
        } else { Err("Invalid compose handle".to_string()) }
    }, |_| JSValue::undefined().bits());
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_restart(handle: f64, services_header: *const StringHeader, promise_ptr: *mut u8) {
    let h = handle as i64;
    let services_json = string_from_header(services_header);
    async_bridge::spawn_for_promise_deferred(promise_ptr, async move {
        let services: Option<Vec<String>> = if !services_json.is_empty() {
            serde_json::from_str(&services_json).ok()
        } else { None };
        if let Some(ch) = handle::get_handle::<ComposeHandle>(h) {
            let engine = ch.0.lock().await;
            engine.restart(services).await.map_err(|e| e.to_string())?;
            Ok(())
        } else { Err("Invalid compose handle".to_string()) }
    }, |_| JSValue::undefined().bits());
}
