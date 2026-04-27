use serde::{Deserialize, Serialize};
use thiserror::Error;
use crate::types::IsolationLevel;

#[derive(Debug, Error, Serialize, Deserialize, Clone)]
#[serde(tag = "type", content = "data")]
pub enum ComposeError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Backend error (code {code}): {message}")]
    BackendError { code: i32, message: String },

    #[error("Verification failed for image {image}: {reason}")]
    VerificationFailed { image: String, reason: String },

    #[error("Dependency cycle detected: {0}")]
    DependencyCycle(String),

    #[error("Service {service} failed to start: {error}")]
    ServiceStartupFailed { service: String, error: String },

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("No container backend found. Probed: {probed:?}")]
    NoBackendFound { probed: Vec<BackendProbeResult> },

    #[error("Backend {name} is not available: {reason}")]
    BackendNotAvailable { name: String, reason: String },

    #[error("Policy violation for node {node}: {reason}")]
    PolicyViolation { node: String, reason: String },

    #[error("Workload ref resolution failed for node {node_id}, projection {projection:?}: {reason}")]
    WorkloadRefResolutionFailed { node_id: String, projection: String, reason: String },

    #[error("Other error: {0}")]
    Other(String),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BackendProbeResult {
    pub name: String,
    pub available: bool,
    pub reason: String,
    pub version: Option<String>,
    pub mode: String, // "local" | "remote"
    pub isolation_level: IsolationLevel,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub message: String,
    pub code: i32,
}

impl ComposeError {
    pub fn to_json(&self) -> String {
        let code = match self {
            ComposeError::NotFound(_) => 404,
            ComposeError::BackendError { code, .. } => *code,
            ComposeError::VerificationFailed { .. } => 403,
            ComposeError::DependencyCycle(_) => 422,
            ComposeError::ServiceStartupFailed { .. } => 500,
            ComposeError::InvalidConfig(_) => 400,
            ComposeError::NoBackendFound { .. } => 503,
            ComposeError::BackendNotAvailable { .. } => 503,
            ComposeError::PolicyViolation { .. } => 403,
            ComposeError::WorkloadRefResolutionFailed { .. } => 500,
            ComposeError::Other(_) => 500,
        };

        let resp = ErrorResponse {
            message: self.to_string(),
            code,
        };
        serde_json::to_string(&resp).unwrap_or_else(|_| r#"{"message":"Unknown error","code":500}"#.to_string())
    }
}
