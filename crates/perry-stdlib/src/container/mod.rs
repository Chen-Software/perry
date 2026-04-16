//! `perry-stdlib` container bridge.

pub mod backend;
pub mod capability;
pub mod compose;
pub mod types;
pub mod verification;

use perry_runtime::{js_promise_new, Promise, StringHeader};
use std::sync::{Arc, OnceLock, Mutex};
use backend::{detect_backend, ContainerBackend};

static BACKEND: OnceLock<Arc<dyn ContainerBackend>> = OnceLock::new();
static BACKEND_INIT_MUTEX: Mutex<()> = Mutex::new(());

fn get_global_backend_instance() -> Result<Arc<dyn ContainerBackend>, String> {
    if let Some(backend) = BACKEND.get() {
        return Ok(Arc::clone(backend));
    }

    let _guard = BACKEND_INIT_MUTEX.lock().unwrap();
    if let Some(backend) = BACKEND.get() {
        return Ok(Arc::clone(backend));
    }

    let backend = tokio::runtime::Handle::current().block_on(async {
        detect_backend().await
            .map(|b| Arc::new(b) as Arc<dyn ContainerBackend>)
            .map_err(|e| format!("No backend found: {:?}", e))
    })?;

    let _ = BACKEND.set(Arc::clone(&backend));
    Ok(backend)
}

unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() || (ptr as usize) < 0x1000 {
        return None;
    }
    let len = (*ptr).byte_len as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    Some(String::from_utf8_lossy(bytes).to_string())
}

#[no_mangle]
pub unsafe extern "C" fn js_container_run(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid JSON".to_string()) });
            return promise;
        }
    };

    let spec: types::ContainerSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e.to_string()) });
            return promise;
        }
    };

    let backend = match get_global_backend_instance() {
        Ok(b) => b,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        match backend.run(&spec).await {
            Ok(handle) => Ok(types::register_container_handle(handle)),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_up(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>("Invalid JSON".to_string()) });
            return promise;
        }
    };

    let spec: types::ComposeSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e.to_string()) });
            return promise;
        }
    };

    let backend = match get_global_backend_instance() {
        Ok(b) => b,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move { Err::<u64, String>(e) });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = perry_container_compose::ComposeEngine::new(spec, "default".into(), backend);
        match engine.up(&[], true, false, false).await {
            Ok(_handle) => Ok(types::register_compose_handle(engine)),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}
