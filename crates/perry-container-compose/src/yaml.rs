use crate::types::ComposeSpec;
use crate::error::ComposeError;
use std::path::Path;

pub fn parse_str(yaml: &str) -> Result<ComposeSpec, ComposeError> {
    serde_yaml::from_str(yaml).map_err(|e| ComposeError::InvalidConfig(e.to_string()))
}

pub fn parse_file(path: impl AsRef<Path>) -> Result<ComposeSpec, ComposeError> {
    let content = std::fs::read_to_string(path).map_err(|e| ComposeError::Other(e.to_string()))?;
    parse_str(&content)
}
