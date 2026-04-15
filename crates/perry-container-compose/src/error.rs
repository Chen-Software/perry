//! Error types for perry-container-compose

use thiserror::Error;

/// Top-level crate error
#[derive(Debug, Error)]
pub enum ComposeError {
    #[error("YAML parse error: {0}")]
    ParseError(#[from] serde_yaml::Error),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Backend error: {0}")]
    BackendError(#[from] BackendError),

    #[error("Validation error: {message}")]
    ValidationError { message: String },

    #[error("Circular dependency detected: {cycle}")]
    CircularDependency { cycle: String },

    #[error("Service not found: {name}")]
    ServiceNotFound { name: String },

    #[error("Compose file not found: {path}")]
    FileNotFound { path: String },

    #[error("Exec error in service '{service}': {message}")]
    ExecError { service: String, message: String },

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

/// Backend (Apple Container / Podman) specific errors
#[derive(Debug, Error)]
pub enum BackendError {
    #[error("Container not found: {name}")]
    NotFound { name: String },

    #[error("Container command failed (exit {code}): {stderr}")]
    CommandFailed { code: i32, stderr: String },

    #[error("Backend not available: {reason}")]
    NotAvailable { reason: String },

    #[error("Image not found: {image}")]
    ImageNotFound { image: String },

    #[error("Build failed: {message}")]
    BuildFailed { message: String },

    #[error("Network error: {message}")]
    NetworkError { message: String },

    #[error("Volume error: {message}")]
    VolumeError { message: String },
}

impl ComposeError {
    pub fn validation(msg: impl Into<String>) -> Self {
        ComposeError::ValidationError {
            message: msg.into(),
        }
    }

    pub fn config(msg: impl Into<String>) -> Self {
        ComposeError::ConfigError(msg.into())
    }
}

pub type Result<T> = std::result::Result<T, ComposeError>;
