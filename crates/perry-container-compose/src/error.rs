use thiserror::Error;
use serde::Serialize;
use crate::types::BackendProbeResult;

#[derive(Debug, Error)]
pub enum ComposeError {
    #[error("Dependency cycle detected: {services:?}")]
    DependencyCycle { services: Vec<String> },
    #[error("Service startup failed: {service}")]
    ServiceStartupFailed { service: String },
    #[error("Image pull failed: {image}")]
    ImagePullFailed { image: String },
    #[error("Backend error: {message} (code: {code})")]
    BackendError { message: String, code: u16 },
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Validation error: {0}")]
    ValidationError(String),
    #[error("Verification failed for image {image}: {reason}")]
    VerificationFailed { image: String, reason: String },
    #[error("No container backend found. Probed: {probed:?}")]
    NoBackendFound { probed: Vec<BackendProbeResult> },
    #[error("Backend not available: {0}")]
    BackendNotAvailable(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("JSON error: {0}")]
    JsonError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("File not found: {0}")]
    FileNotFound(String),
}

#[derive(Serialize)]
struct ErrorResponse {
    message: String,
    code: u16,
}

impl ComposeError {
    pub fn http_code(&self) -> u16 {
        match self {
            ComposeError::DependencyCycle { .. } => 422,
            ComposeError::ServiceStartupFailed { .. } => 500,
            ComposeError::ImagePullFailed { .. } => 500,
            ComposeError::BackendError { code, .. } => *code,
            ComposeError::NotFound(_) => 404,
            ComposeError::ValidationError(_) => 400,
            ComposeError::VerificationFailed { .. } => 403,
            ComposeError::NoBackendFound { .. } => 503,
            ComposeError::BackendNotAvailable(_) => 503,
            ComposeError::ParseError(_) => 400,
            ComposeError::JsonError(_) => 400,
            ComposeError::IoError(_) => 500,
            ComposeError::FileNotFound(_) => 404,
        }
    }

    pub fn to_json(&self) -> String {
        let resp = ErrorResponse {
            message: self.to_string(),
            code: self.http_code(),
        };
        serde_json::to_string(&resp).unwrap_or_else(|_| {
            format!(r#"{{"message": "{}", "code": {}}}"#, self, self.http_code())
        })
    }
}
