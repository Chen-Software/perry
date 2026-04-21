use std::sync::{OnceLock, RwLock};
use std::collections::HashMap;
use crate::container::error::ContainerError;

pub const CHAINGUARD_IDENTITY: &str = "https://github.com/chainguard-images/images/.github/workflows/release.yaml@refs/heads/main";
pub const CHAINGUARD_ISSUER: &str = "https://token.actions.githubusercontent.com";

static VERIFICATION_CACHE: OnceLock<RwLock<HashMap<String, String>>> = OnceLock::new();

pub fn verify_image(reference: &str) -> Result<String, ContainerError> {
    // Basic placeholder for image verification
    Ok(reference.to_string())
}

pub fn get_default_base_image() -> &'static str {
    "cgr.dev/chainguard/alpine-base"
}

pub fn get_chainguard_image(tool: &str) -> Option<String> {
    match tool {
        "node" => Some("cgr.dev/chainguard/node:latest".to_string()),
        "python" => Some("cgr.dev/chainguard/python:latest".to_string()),
        _ => None,
    }
}
