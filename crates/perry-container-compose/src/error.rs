use serde::{Serialize, Deserialize};
use thiserror::Error;

#[derive(Debug, Error, Serialize, Deserialize)]
pub enum ComposeError {
    #[error("Dependency cycle detected: {services:?}")]
    DependencyCycle { services: Vec<String> },
    #[error("Service startup failed: {service}")]
    ServiceStartupFailed { service: String, error: String },
    #[error("Image pull failed: {service} ({image}): {message}")]
    ImagePullFailed { service: String, image: String, message: String },
    #[error("Backend error: {message} (code: {code})")]
    BackendError { code: i32, message: String },
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("JSON error: {0}")]
    JsonError(String),
    #[error("IO error: {0}")]
    IoError(String),
    #[error("Validation error: {0}")]
    ValidationError(String),
    #[error("Verification failed for {image}: {reason}")]
    VerificationFailed { image: String, reason: String },
    #[error("Invalid config: {0}")]
    InvalidConfig(String),
    #[error("File not found: {0}")]
    FileNotFound(String),
    #[error("No backend found")]
    NoBackendFound { probed: Vec<BackendProbeResult> },
    #[error("Backend {name} not available: {reason}")]
    BackendNotAvailable { name: String, reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendProbeResult {
    pub name: String,
    pub available: bool,
    pub reason: Option<String>,
    pub version: Option<String>,
}

pub type Result<T> = std::result::Result<T, ComposeError>;

pub fn compose_error_to_js(err: &ComposeError) -> String {
    let (message, code) = match err {
        ComposeError::NotFound(_) | ComposeError::FileNotFound(_) => (err.to_string(), 404),
        ComposeError::ParseError(_) | ComposeError::JsonError(_) | ComposeError::ValidationError(_) | ComposeError::InvalidConfig(_) => (err.to_string(), 400),
        ComposeError::DependencyCycle { .. } => (err.to_string(), 422),
        ComposeError::VerificationFailed { .. } => (err.to_string(), 403),
        ComposeError::NoBackendFound { .. } | ComposeError::BackendNotAvailable { .. } => (err.to_string(), 503),
        ComposeError::BackendError { code, .. } => (err.to_string(), *code),
        _ => (err.to_string(), 500),
    };
    format!("{{\"message\": {:?}, \"code\": {}}}", message, code)
}
