use thiserror::Error;
use serde::{Serialize, Deserialize};

#[derive(Debug, Error, Serialize, Deserialize)]
pub enum ContainerError {
    #[error("Backend error: {message} (code: {code})")]
    BackendError { code: i32, message: String },
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Verification failed for {image}: {reason}")]
    VerificationFailed { image: String, reason: String },
    #[error("No backend found")]
    NoBackendFound { probed: Vec<perry_container_compose::error::BackendProbeResult> },
    #[error("Backend {name} not available: {reason}")]
    BackendNotAvailable { name: String, reason: String },
}
