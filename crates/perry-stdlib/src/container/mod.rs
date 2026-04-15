//! Perry container module FFI bridge.

pub mod backend;
pub mod capability;
pub mod compose;
pub mod types;
pub mod verification;

use perry_container_compose::backend::{detect_backend, ContainerBackend};
use perry_runtime::{js_promise_new, Promise, StringHeader};
use std::sync::{Arc, OnceLock};
use crate::container::types::*;

pub(crate) mod mod_private {
    use super::*;
    pub static BACKEND: OnceLock<Arc<dyn ContainerBackend + Send + Sync>> = OnceLock::new();

    pub async fn get_global_backend_instance() -> Result<Arc<dyn ContainerBackend + Send + Sync>, String> {
        if let Some(b) = BACKEND.get() {
            return Ok(Arc::clone(b));
        }

        let backend_res = detect_backend().await;

        match backend_res {
            Ok(b) => {
                let arc_b = Arc::new(b) as Arc<dyn ContainerBackend + Send + Sync>;
                let _ = BACKEND.set(Arc::clone(&arc_b));
                Ok(arc_b)
            }
            Err(probed) => Err(format!("No backend found: {:?}", probed)),
        }
    }
}

use mod_private::get_global_backend_instance;

#[no_mangle]
pub unsafe extern "C" fn js_container_run(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid spec JSON".to_string())
            });
            return promise;
        }
    };

    let spec: ContainerSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(format!("Invalid ContainerSpec: {}", e))
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = get_global_backend_instance().await.map_err(|e| e.to_string())?;
        let internal_spec = perry_container_compose::types::ContainerSpec {
            image: spec.image,
            name: spec.name,
            ports: spec.ports,
            volumes: spec.volumes,
            env: spec.env,
            cmd: spec.cmd,
            entrypoint: spec.entrypoint,
            network: spec.network,
            rm: spec.rm,
            read_only: spec.read_only,
        };
        let handle = backend.run(&internal_spec).await.map_err(|e| e.to_string())?;
        let id = register_container_handle(ContainerHandle { id: handle.id, name: handle.name });
        Ok(id)
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_composeUp(spec_json_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();
    let spec_json = match string_from_header(spec_json_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid spec JSON".to_string())
            });
            return promise;
        }
    };

    let spec: perry_container_compose::types::ComposeSpec = match serde_json::from_str(&spec_json) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(format!("Invalid ComposeSpec: {}", e))
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let handle = compose::compose_up(spec).await.map_err(|e| e.to_string())?;
        Ok(handle.stack_id)
    });

    promise
}

#[no_mangle]
pub unsafe extern "C" fn js_container_getBackend() -> *const StringHeader {
    if let Some(backend) = mod_private::BACKEND.get() {
        let name = backend.backend_name();
        return perry_runtime::js_string_from_bytes(name.as_ptr(), name.len() as u32);
    }
    std::ptr::null()
}
