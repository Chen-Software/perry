use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
use super::types::ContainerError;
use super::get_global_backend;

pub const CHAINGUARD_IDENTITY: &str =
    "https://github.com/chainguard-images/images/.github/workflows/sign.yaml@refs/heads/main";
pub const CHAINGUARD_ISSUER: &str =
    "https://token.actions.githubusercontent.com";

#[derive(Debug, Clone)]
enum VerificationResult {
    Verified(String), // digest
    Failed(String),   // reason
}

static VERIFICATION_CACHE: OnceLock<RwLock<HashMap<String, VerificationResult>>> = OnceLock::new();

pub async fn fetch_image_digest(reference: &str) -> Result<String, ContainerError> {
    let backend = get_global_backend().await?;
    let info = backend.inspect_image(reference).await
        .map_err(|e| ContainerError::from(e))?;

    // Attempt to find digest in common locations
    let digest = info.get("RepoDigests")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|v| v.as_str())
        .and_then(|s| s.split('@').last())
        .or_else(|| info.get("Id").and_then(|v| v.as_str()))
        .map(|s| s.to_string())
        .ok_or_else(|| ContainerError::NotFound(format!("Digest for {}", reference)))?;

    Ok(digest)
}

pub async fn run_cosign_verify(reference: &str, digest: &str) -> VerificationResult {
    // In a real implementation, we would shell out to `cosign verify`
    // For now, we simulate success for Chainguard images
    if reference.starts_with("cgr.dev/chainguard/") {
        VerificationResult::Verified(digest.to_string())
    } else {
        VerificationResult::Failed("Not a Chainguard image".to_string())
    }
}

pub async fn verify_image(reference: &str) -> Result<String, ContainerError> {
    let digest = fetch_image_digest(reference).await?;

    let cache = VERIFICATION_CACHE.get_or_init(|| RwLock::new(HashMap::new()));
    {
        let read = cache.read().unwrap();
        if let Some(res) = read.get(&digest) {
            return match res {
                VerificationResult::Verified(d) => Ok(d.clone()),
                VerificationResult::Failed(r) => Err(ContainerError::VerificationFailed {
                    image: reference.to_string(),
                    reason: r.clone(),
                }),
            };
        }
    }

    let result = run_cosign_verify(reference, &digest).await;

    {
        let mut write = cache.write().unwrap();
        write.insert(digest.clone(), result.clone());
    }

    match result {
        VerificationResult::Verified(d) => Ok(d),
        VerificationResult::Failed(r) => Err(ContainerError::VerificationFailed {
            image: reference.to_string(),
            reason: r,
        }),
    }
}

pub fn get_chainguard_image(tool: &str) -> Option<String> {
    let map = HashMap::from([
        ("python", "cgr.dev/chainguard/python:latest"),
        ("node", "cgr.dev/chainguard/node:latest"),
        ("go", "cgr.dev/chainguard/go:latest"),
    ]);
    map.get(tool).map(|s| s.to_string())
}

pub fn get_default_base_image() -> &'static str {
    "cgr.dev/chainguard/alpine-base:latest"
}
