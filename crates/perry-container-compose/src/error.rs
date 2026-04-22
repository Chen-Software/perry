use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendProbeResult {
    pub name: String,
    pub available: bool,
    pub reason: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ComposeError {
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

pub type Result<T> = std::result::Result<T, ComposeError>;

impl ComposeError {
    pub fn validation(message: String) -> Self {
        ComposeError::ValidationError { message }
    }

    pub fn to_js_json(&self) -> String {
        let code = match self {
            ComposeError::NotFound(_) => 404,
            ComposeError::BackendError { code, .. } => *code,
            ComposeError::DependencyCycle { .. } => 422,
            ComposeError::ValidationError { .. } => 400,
            ComposeError::ParseError(_) => 400,
            ComposeError::JsonError(_) => 400,
            ComposeError::VerificationFailed { .. } => 403,
            ComposeError::NoBackendFound { .. } => 503,
            ComposeError::BackendNotAvailable { .. } => 503,
            ComposeError::FileNotFound { .. } => 404,
            _ => 500,
        };
        serde_json::json!({
            "message": self.to_string(),
            "code": code
        }).to_string()
    }
}
