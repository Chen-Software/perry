use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, Serialize, Deserialize, Clone)]
#[serde(tag = "type", content = "data")]
pub enum ComposeError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Backend error (code {code}): {message}")]
    BackendError { code: i32, message: String },

    #[error("Verification failed for image {image}: {reason}")]
    VerificationFailed { image: String, reason: String },

    #[error("Dependency cycle detected involving: {cycle:?}")]
    DependencyCycle { cycle: Vec<String> },

    #[error("Service '{service}' failed to start: {error}")]
    ServiceStartupFailed { service: String, error: String },

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("No container backend found. Probed: {probed:?}")]
    NoBackendFound { probed: Vec<BackendProbeResult> },

    #[error("Backend '{name}' not available: {reason}")]
    BackendNotAvailable { name: String, reason: String },

    #[error("WorkloadRef resolution failed for node '{node_id}' (projection: {projection:?}): {reason}")]
    WorkloadRefResolutionFailed {
        node_id: String,
        projection: String,
        reason: String,
    },

    #[error("Policy violation in node '{node}': required {required}, available {available}")]
    PolicyViolation {
        node: String,
        required: String,
        available: String,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BackendProbeResult {
    pub name: String,
    pub available: bool,
    pub reason: String,
}

impl ComposeError {
    pub fn to_json(&self) -> String {
        serde_json::json!({
            "message": self.to_string(),
            "code": self.exit_code()
        })
        .to_string()
    }

    pub fn exit_code(&self) -> i32 {
        match self {
            Self::NotFound(_) => 404,
            Self::BackendError { code, .. } => *code,
            Self::VerificationFailed { .. } => 403,
            Self::DependencyCycle { .. } => 422,
            Self::ServiceStartupFailed { .. } => 500,
            Self::InvalidConfig(_) => 400,
            Self::NoBackendFound { .. } => 503,
            Self::BackendNotAvailable { .. } => 503,
            Self::WorkloadRefResolutionFailed { .. } => 500,
            Self::PolicyViolation { .. } => 403,
        }
    }
}
