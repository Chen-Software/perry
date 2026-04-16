use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
use crate::container::types::ContainerError;

pub const CHAINGUARD_IDENTITY: &str = "https://github.com/chainguard-images/images/.github/workflows/sign.yaml@refs/heads/main";
pub const CHAINGUARD_ISSUER: &str = "https://token.actions.githubusercontent.com";

#[derive(Debug, Clone)]
pub enum VerificationResult { Verified, Failed(String) }

static VERIFICATION_CACHE: OnceLock<RwLock<HashMap<String, VerificationResult>>> = OnceLock::new();

pub async fn verify_image(reference: &str) -> Result<String, ContainerError> {
    let cache = VERIFICATION_CACHE.get_or_init(|| RwLock::new(HashMap::new()));
    {
        let r = cache.read().unwrap();
        if let Some(res) = r.get(reference) {
            return match res {
                VerificationResult::Verified => Ok(reference.to_string()),
                VerificationResult::Failed(s) => Err(ContainerError::VerificationFailed { image: reference.into(), reason: s.clone() }),
            };
        }
    }
    // Stub implementation
    Ok(reference.to_string())
}

pub fn get_chainguard_image(tool: &str) -> Option<String> {
    match tool {
        "git" => Some("cgr.dev/chainguard/git".to_string()),
        "curl" => Some("cgr.dev/chainguard/curl".to_string()),
        _ => None,
    }
}

pub fn get_default_base_image() -> &'static str { "cgr.dev/chainguard/alpine-base" }
