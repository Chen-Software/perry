//! Image verification via Sigstore/cosign.

use super::types::ContainerError;
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

pub const CHAINGUARD_IDENTITY: &str =
    "https://github.com/chainguard-images/images/.github/workflows/sign.yaml@refs/heads/main";
pub const CHAINGUARD_ISSUER: &str = "https://token.actions.githubusercontent.com";

#[derive(Debug, Clone)]
pub enum VerificationResult {
    Verified,
    Failed(String),
}

static VERIFICATION_CACHE: OnceLock<RwLock<HashMap<String, VerificationResult>>> = OnceLock::new();

pub async fn verify_image(reference: &str) -> Result<String, ContainerError> {
    // 1. Fetch digest (simplified placeholder)
    let digest = format!("sha256:{}", reference);

    let cache = VERIFICATION_CACHE.get_or_init(|| RwLock::new(HashMap::new()));
    {
        let read = cache.read().unwrap();
        if let Some(res) = read.get(&digest) {
            match res {
                VerificationResult::Verified => return Ok(digest),
                VerificationResult::Failed(e) => {
                    return Err(ContainerError::VerificationFailed {
                        image: reference.to_string(),
                        reason: e.clone(),
                    })
                }
            }
        }
    }

    // 2. Run cosign verify (simplified placeholder)
    let res = VerificationResult::Verified;
    cache.write().unwrap().insert(digest.clone(), res);

    Ok(digest)
}

pub fn get_default_base_image() -> &'static str {
    "cgr.dev/chainguard/alpine-base"
}

pub fn get_chainguard_image(tool: &str) -> Option<String> {
    match tool {
        "git" => Some("cgr.dev/chainguard/git".to_string()),
        "node" => Some("cgr.dev/chainguard/node".to_string()),
        _ => None,
    }
}
