//! Error types for perry-container-compose.
//!
//! Defines the canonical `ComposeError` enum and FFI error mapping.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Result of probing a single container backend candidate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendProbeResult {
    pub name: String,
    pub available: bool,
    pub reason: String,
}

/// Top-level crate error
#[derive(Debug, Error, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
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
    ParseError(#[serde(serialize_with = "serialize_error")] serde_yaml::Error),

    #[error("JSON error: {0}")]
    JsonError(#[serde(serialize_with = "serialize_error")] serde_json::Error),

    #[error("I/O error: {0}")]
    IoError(#[serde(serialize_with = "serialize_error")] std::io::Error),

    #[error("Validation error: {message}")]
    ValidationError { message: String },

    #[error("Image verification failed for '{image}': {reason}")]
    VerificationFailed { image: String, reason: String },

    #[error("File not found: {path}")]
    FileNotFound { path: String },

    #[error("No container backend found. Probed: {probed:?}")]
    NoBackendFound { probed: Vec<BackendProbeResult> },

    #[error("Backend '{name}' is not available: {reason}")]
    BackendNotAvailable { name: String, reason: String },
}

fn serialize_error<S, E>(e: &E, serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
    E: std::fmt::Display,
{
    serializer.serialize_str(&e.to_string())
}

impl From<serde_yaml::Error> for ComposeError {
    fn from(e: serde_yaml::Error) -> Self {
        Self::ParseError(e)
    }
}

impl From<serde_json::Error> for ComposeError {
    fn from(e: serde_json::Error) -> Self {
        Self::JsonError(e)
    }
}

impl From<std::io::Error> for ComposeError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e)
    }
}

impl ComposeError {
    pub fn validation(msg: impl Into<String>) -> Self {
        ComposeError::ValidationError {
            message: msg.into(),
        }
    }
}

pub type Result<T> = std::result::Result<T, ComposeError>;

/// Convert a `ComposeError` to a JSON string `{ "message": "...", "code": N }`
/// suitable for passing across the FFI boundary.
pub fn compose_error_to_js(e: &ComposeError) -> String {
    let code = match e {
        ComposeError::NotFound(_) => 404,
        ComposeError::BackendError { code, .. } => *code,
        ComposeError::DependencyCycle { .. } => 422,
        ComposeError::ValidationError { .. } => 400,
        ComposeError::VerificationFailed { .. } => 403,
        ComposeError::NoBackendFound { .. } => 503,
        ComposeError::BackendNotAvailable { .. } => 503,
        _ => 500,
    };
    serde_json::json!({
        "message": e.to_string(),
        "code": code
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes() {
        let err = ComposeError::NotFound("foo".into());
        assert_eq!(compose_error_to_js(&err).contains("\"code\":404"), true);

        let err = ComposeError::DependencyCycle {
            services: vec!["a".into()],
        };
        assert_eq!(compose_error_to_js(&err).contains("\"code\":422"), true);

        let err = ComposeError::ValidationError {
            message: "bad".into(),
        };
        assert_eq!(compose_error_to_js(&err).contains("\"code\":400"), true);

        let err = ComposeError::VerificationFailed {
            image: "img".into(),
            reason: "fail".into(),
        };
        assert_eq!(compose_error_to_js(&err).contains("\"code\":403"), true);

        let err = ComposeError::ParseError(serde_yaml::from_str::<serde_yaml::Value>("bad: [1,2").unwrap_err());
        assert_eq!(compose_error_to_js(&err).contains("\"code\":500"), true);

        let err = ComposeError::NoBackendFound {
            probed: vec![BackendProbeResult {
                name: "docker".into(),
                available: false,
                reason: "not found".into(),
            }],
        };
        assert_eq!(compose_error_to_js(&err).contains("\"code\":503"), true);

        let err = ComposeError::BackendNotAvailable {
            name: "podman".into(),
            reason: "machine not running".into(),
        };
        assert_eq!(compose_error_to_js(&err).contains("\"code\":503"), true);
    }
}
