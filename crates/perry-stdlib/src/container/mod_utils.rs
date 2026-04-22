//! Internal utilities for container module FFI

use crate::container::types::ComposeError;
use crate::container::backend::{detect_backend, ContainerBackend};
use std::sync::{Arc, OnceLock};

static BACKEND: OnceLock<Arc<dyn ContainerBackend + Send + Sync>> = OnceLock::new();

pub fn backend_err_to_js(msg: String) -> String {
    serde_json::json!({
        "message": msg,
        "code": 503
    }).to_string()
}

pub async fn get_global_backend_instance() -> Result<Arc<dyn ContainerBackend + Send + Sync>, String> {
    if let Some(b) = BACKEND.get() {
        return Ok(Arc::clone(b));
    }

    match detect_backend().await {
        Ok(b) => {
            let arc_b: Arc<dyn ContainerBackend + Send + Sync> = Arc::from(b) as Arc<dyn ContainerBackend + Send + Sync>;
            let _ = BACKEND.set(Arc::clone(&arc_b));
            Ok(arc_b)
        }
        Err(e) => Err(format!("{:?}", e)),
    }
}

pub fn get_backend_if_initialized() -> Option<Arc<dyn ContainerBackend + Send + Sync>> {
    BACKEND.get().cloned()
}
