use perry_container_compose::error::{BackendProbeResult, ComposeError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredBackendProbeResult {
    pub name: String,
    pub available: bool,
    pub reason: String,
}

impl From<BackendProbeResult> for RegisteredBackendProbeResult {
    fn from(res: BackendProbeResult) -> Self {
        Self {
            name: res.name,
            available: res.available,
            reason: res.reason,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ContainerError {
    #[error("Dependency cycle detected in services: {services:?}")]
    DependencyCycle { services: Vec<String> },

    #[error("Service '{service}' failed to start: {message}")]
    ServiceStartupFailed { service: String, message: String },

    #[error("Backend error (exit {code}): {message}")]
    BackendError { code: i32, message: String },

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Parse error: {0}")]
    ParseError(#[from] serde_yaml::Error),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Validation error: {message}")]
    ValidationError { message: String },

    #[error("Image verification failed for '{image}': {reason}")]
    VerificationFailed { image: String, reason: String },

    #[error("File not found: {path}")]
    FileNotFound { path: String },

    #[error("No container backend found. Probed: {probed:?}")]
    NoBackendFound { probed: Vec<BackendProbeResult> },

    #[error("Specified backend '{name}' is not available: {reason}")]
    BackendNotAvailable { name: String, reason: String },
}

impl From<ComposeError> for ContainerError {
    fn from(e: ComposeError) -> Self {
        match e {
            ComposeError::DependencyCycle { services } => ContainerError::DependencyCycle { services },
            ComposeError::ServiceStartupFailed { service, message } => ContainerError::ServiceStartupFailed { service, message },
            ComposeError::BackendError { code, message } => ContainerError::BackendError { code, message },
            ComposeError::NotFound(s) => ContainerError::NotFound(s),
            ComposeError::ParseError(e) => ContainerError::ParseError(e),
            ComposeError::JsonError(e) => ContainerError::JsonError(e),
            ComposeError::IoError(e) => ContainerError::IoError(e),
            ComposeError::ValidationError { message } => ContainerError::ValidationError { message },
            ComposeError::VerificationFailed { image, reason } => ContainerError::VerificationFailed { image, reason },
            ComposeError::FileNotFound { path } => ContainerError::FileNotFound { path },
            ComposeError::NoBackendFound { probed } => ContainerError::NoBackendFound { probed },
            ComposeError::BackendNotAvailable { name, reason } => ContainerError::BackendNotAvailable { name, reason },
        }
    }
}
