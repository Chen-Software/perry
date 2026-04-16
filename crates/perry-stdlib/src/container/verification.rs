use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
use crate::container::types::ContainerError;

pub const CHAINGUARD_IDENTITY: &str = "https://github.com/chainguard-images/images/.github/workflows/sign.yaml@refs/heads/main";
pub const CHAINGUARD_ISSUER: &str = "https://token.actions.githubusercontent.com";

#[derive(Debug, Clone)]
pub enum VerificationResult {
    Verified(String),
    Failed(String)
}

static VERIFICATION_CACHE: OnceLock<RwLock<HashMap<String, VerificationResult>>> = OnceLock::new();

pub async fn verify_image(reference: &str) -> Result<String, ContainerError> {
    // 1. Fetch digest (tag -> digest resolution)
    let digest = fetch_image_digest(reference).await?;

    // 2. Check cache (keyed by digest)
    let cache = VERIFICATION_CACHE.get_or_init(|| RwLock::new(HashMap::new()));
    {
        let r = cache.read().unwrap();
        if let Some(res) = r.get(&digest) {
            return match res {
                VerificationResult::Verified(d) => Ok(d.clone()),
                VerificationResult::Failed(s) => Err(ContainerError::VerificationFailed {
                    image: reference.into(),
                    reason: s.clone()
                }),
            };
        }
    }

    // 3. Run cosign verify (Stubbed for now)
    let result = run_cosign_verify(reference, &digest).await;

    // 4. Cache result
    {
        let mut w = cache.write().unwrap();
        w.insert(digest.clone(), result.clone());
    }

    match result {
        VerificationResult::Verified(d) => Ok(d),
        VerificationResult::Failed(s) => Err(ContainerError::VerificationFailed {
            image: reference.into(),
            reason: s
        }),
    }
}

async fn fetch_image_digest(reference: &str) -> Result<String, ContainerError> {
    // Simplified: in a real implementation, this would call 'docker inspect' or a registry API
    // Returning a dummy digest for now to satisfy the flow
    Ok(format!("sha256:{:064x}", rand::random::<u128>()))
}

async fn run_cosign_verify(_reference: &str, digest: &str) -> VerificationResult {
    // In production, this would execute 'cosign verify ...'
    VerificationResult::Verified(digest.to_string())
}

pub fn get_chainguard_image(tool: &str) -> Option<String> {
    match tool {
        "git" => Some("cgr.dev/chainguard/git".to_string()),
        "curl" => Some("cgr.dev/chainguard/curl".to_string()),
        "wget" => Some("cgr.dev/chainguard/wget".to_string()),
        "openssl" => Some("cgr.dev/chainguard/openssl".to_string()),
        "bash" => Some("cgr.dev/chainguard/bash".to_string()),
        "sh" => Some("cgr.dev/chainguard/busybox".to_string()),
        "node" => Some("cgr.dev/chainguard/node".to_string()),
        "python" => Some("cgr.dev/chainguard/python".to_string()),
        "ruby" => Some("cgr.dev/chainguard/ruby".to_string()),
        "go" => Some("cgr.dev/chainguard/go".to_string()),
        "rust" => Some("cgr.dev/chainguard/rust".to_string()),
        _ => None,
    }
}

pub fn get_default_base_image() -> &'static str { "cgr.dev/chainguard/alpine-base" }
