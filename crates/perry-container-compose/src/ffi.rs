//! FFI exports for standalone linking

use crate::types::ComposeSpec;
use crate::error::ComposeError;
use crate::compose::ComposeEngine;
use crate::backend::get_backend;
use std::sync::Arc;
use dashmap::DashMap;
use once_cell::sync::Lazy;

static COMPOSE_ENGINES: Lazy<DashMap<u64, Arc<ComposeEngine>>> = Lazy::new(DashMap::new);

#[no_mangle]
pub unsafe extern "C" fn js_compose_up_internal(spec_json: *const u8, len: usize) -> i64 {
    let bytes = std::slice::from_raw_parts(spec_json, len);
    let spec: ComposeSpec = match serde_json::from_slice(bytes) {
        Ok(s) => s,
        Err(_) => return -1,
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let backend = match get_backend().await {
            Ok(b) => Arc::from(b),
            Err(_) => return -2,
        };
        let engine = ComposeEngine::new(spec, backend);
        match engine.up().await {
            Ok(handle) => {
                let id = handle.stack_id;
                COMPOSE_ENGINES.insert(id, Arc::new(engine));
                id as i64
            }
            Err(_) => -3,
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_down_internal(stack_id: i64, volumes: i32) -> i32 {
    let id = stack_id as u64;
    let engine = match COMPOSE_ENGINES.get(&id) {
        Some(e) => Arc::clone(e.value()),
        None => return -1,
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        match engine.down(volumes != 0).await {
            Ok(()) => {
                COMPOSE_ENGINES.remove(&id);
                0
            }
            Err(_) => -2,
        }
    })
}
