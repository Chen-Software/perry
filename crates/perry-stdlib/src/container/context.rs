use std::sync::{Arc, OnceLock};
use perry_container_compose::backend::ContainerBackend;

pub struct ContainerContext {
    pub backend: OnceLock<Arc<dyn ContainerBackend>>,
}

impl ContainerContext {
    pub fn new() -> Self {
        Self {
            backend: OnceLock::new(),
        }
    }

    pub fn global() -> &'static ContainerContext {
        static GLOBAL: OnceLock<ContainerContext> = OnceLock::new();
        GLOBAL.get_or_init(|| ContainerContext::new())
    }

    pub async fn get_backend(&self) -> Result<Arc<dyn ContainerBackend>, String> {
        if let Some(b) = self.backend.get() {
            return Ok(Arc::clone(b));
        }

        let b = perry_container_compose::backend::detect_backend().await
            .map_err(|probed| format!("No backend found: {:?}", probed))?;

        let backend: Arc<dyn ContainerBackend> = Arc::from(b);
        let _ = self.backend.set(Arc::clone(&backend));
        Ok(backend)
    }
}
