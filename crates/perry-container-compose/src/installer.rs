use crate::error::ComposeError;
use crate::backend::BackendDriver;

pub struct BackendInstaller;

impl BackendInstaller {
    pub async fn run() -> Result<BackendDriver, ComposeError> {
        // In real implementation, this would use dialoguer to prompt user
        Err(ComposeError::NoBackendFound { probed: vec![] })
    }
}
