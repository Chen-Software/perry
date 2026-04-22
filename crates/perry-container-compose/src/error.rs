use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum ComposeError {
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Backend error (exit {code}): {message}")]
    BackendError { code: i32, message: String },
    #[error("Image verification failed for '{image}': {reason}")]
    VerificationFailed { image: String, reason: String },
    #[error("Dependency cycle detected in services: {services:?}")]
    DependencyCycle { services: Vec<String> },
    #[error("Service '{service}' failed to start: {message}")]
    ServiceStartupFailed { service: String, message: String },
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("No container backend found. Probed: {probed:?}")]
    NoBackendFound { probed: Vec<BackendProbeResult> },
    #[error("Specified backend '{name}' is not available: {reason}")]
    BackendNotAvailable { name: String, reason: String },
    #[error("Validation error: {message}")]
    ValidationError { message: String },
    #[error("Parse error: {0}")]
    ParseError(#[from] serde_yaml::Error),
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("File not found: {path}")]
    FileNotFound { path: String },
    #[error("Policy violation for node '{node}': required {required:?}, available {available:?}")]
    PolicyViolation { node: String, required: crate::types::IsolationLevel, available: crate::types::IsolationLevel },
    #[error("Workload reference resolution failed for node '{node_id}', projection '{projection}': {reason}")]
    WorkloadRefResolutionFailed { node_id: String, projection: String, reason: String },
}

pub type Result<T> = std::result::Result<T, ComposeError>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BackendProbeResult {
    pub name: String,
    pub available: bool,
    pub reason: String,
}

pub fn compose_error_to_json(e: &ComposeError) -> String {
    let code = match e {
        ComposeError::NotFound(_) => 404,
        ComposeError::VerificationFailed { .. } => 403,
        ComposeError::DependencyCycle { .. } => 422,
        ComposeError::ServiceStartupFailed { .. } => 500,
        ComposeError::InvalidConfig(_) | ComposeError::ValidationError { .. } | ComposeError::ParseError(_) => 400,
        ComposeError::NoBackendFound { .. } | ComposeError::BackendNotAvailable { .. } => 503,
        ComposeError::BackendError { code, .. } => *code,
        _ => 500,
    };
    serde_json::json!({
        "message": e.to_string(),
        "code": code
    }).to_string()
}
