use crate::error::{ComposeError, BackendProbeResult};
use crate::backend::{BackendDriver, ContainerBackend, detect_backend};
use std::sync::Arc;

pub struct BackendInstaller;

impl BackendInstaller {
    pub async fn run() -> Result<Arc<dyn ContainerBackend>, ComposeError> {
        #[cfg(feature = "installer")]
        {
            if !console::Term::stderr().is_term() {
                return Err(ComposeError::NoBackendFound { probed: vec![] });
            }
            // Implementation of interactive installer
            println!("Perry needs a container runtime to continue.");
            // ...
        }

        Err(ComposeError::NoBackendFound { probed: vec![] })
    }
}
