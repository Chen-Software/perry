use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendProbeResult {
    pub name: String,
    pub available: bool,
    pub reason: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ComposeError {
    #[error("Dependency cycle detected in services: {services:?}")]
    DependencyCycle { services: Vec<String> },

    #[error("Service '{service}' failed to start: {message}")]
    ServiceStartupFailed { service: String, message: String },

    #[error("Image pull failed for service '{service}' (image '{image}'): {message}")]
    ImagePullFailed { service: String, image: String, message: String },

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

pub type Result<T> = std::result::Result<T, ComposeError>;

impl ComposeError {
    pub fn validation(message: String) -> Self {
        ComposeError::ValidationError { message }
    }
}

pub fn compose_error_to_js(err: ComposeError) -> serde_json::Value {
    let (message, code) = match err {
        ComposeError::NotFound(_) | ComposeError::FileNotFound { .. } => (err.to_string(), 404),
        ComposeError::ParseError(_) | ComposeError::JsonError(_) | ComposeError::ValidationError { .. } => (err.to_string(), 400),
        ComposeError::DependencyCycle { .. } => (err.to_string(), 422),
        ComposeError::VerificationFailed { .. } => (err.to_string(), 403),
        ComposeError::NoBackendFound { .. } | ComposeError::BackendNotAvailable { .. } => (err.to_string(), 503),
        _ => (err.to_string(), 500),
    };
    serde_json::json!({
        "message": message,
        "code": code
    })
}
