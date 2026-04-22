//! Sigstore/cosign OCI image verification.

use crate::container::types::FfiError;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
use tokio::process::Command;

pub const CHAINGUARD_IDENTITY: &str =
    "https://github.com/chainguard-images/images/.github/workflows/sign.yaml@refs/heads/main";
pub const CHAINGUARD_ISSUER: &str =
    "https://token.actions.githubusercontent.com";

#[derive(Debug, Clone)]
pub enum VerificationResult {
    Verified(String), // Returns the digest
    Failed(String),
}

static VERIFICATION_CACHE: Lazy<RwLock<HashMap<String, VerificationResult>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

/// Verify an OCI image via Sigstore/cosign keyless verification.
pub async fn verify_image(reference: &str) -> Result<String, String> {
    // 1. Fetch digest (e.g. via backend inspect)
    let digest = fetch_image_digest(reference).await?;

    // 2. Check cache
    {
        let cache = VERIFICATION_CACHE.read().unwrap();
        if let Some(result) = cache.get(&digest) {
            return match result {
                VerificationResult::Verified(d) => Ok(d.clone()),
                VerificationResult::Failed(r) => Err(r.clone()),
            };
        }
    }

    // 3. Run cosign verify
    let result = run_cosign_verify(reference, &digest).await;

    // 4. Cache and return
    {
        let mut cache = VERIFICATION_CACHE.write().unwrap();
        cache.insert(digest.clone(), result.clone());
    }

    match result {
        VerificationResult::Verified(d) => Ok(d),
        VerificationResult::Failed(r) => Err(r),
    }
}

async fn fetch_image_digest(reference: &str) -> Result<String, String> {
    let backend = match crate::container::get_backend_instance().await {
        Ok(b) => b,
        Err(e) => return Err(format!("failed to get backend: {}", e)),
    };

    let info = backend.inspect(reference).await.map_err(|e| e.to_string())?;
    // For many backends, we might need a more specific way to get the digest.
    // Docker-compatible inspect should return ID which is often the digest.
    if info.id.starts_with("sha256:") {
        Ok(info.id)
    } else {
        // Fallback or retry with specific format
        let output = Command::new(backend.backend_name())
            .args(["inspect", "--format", "{{index .RepoDigests 0}}", reference])
            .output()
            .await
            .map_err(|e| e.to_string())?;

        if output.status.success() {
            let out = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if let Some((_, digest)) = out.split_once('@') {
                return Ok(digest.to_string());
            }
        }
        Err(format!("failed to fetch digest for {}", reference))
    }
}

async fn run_cosign_verify(reference: &str, _digest: &str) -> VerificationResult {
    // cosign verify --certificate-identity ... --certificate-oidc-issuer ... <image>
    let output = Command::new("cosign")
        .args([
            "verify",
            "--certificate-identity", CHAINGUARD_IDENTITY,
            "--certificate-oidc-issuer", CHAINGUARD_ISSUER,
            reference,
        ])
        .output()
        .await;

    match output {
        Ok(out) if out.status.success() => {
            // cosign verify prints JSON to stdout on success (or a success message to stderr)
            VerificationResult::Verified("verified".into())
        }
        Ok(out) => VerificationResult::Failed(String::from_utf8_lossy(&out.stderr).to_string()),
        Err(e) => VerificationResult::Failed(e.to_string()),
    }
}

pub fn get_chainguard_image(tool: &str) -> Option<String> {
    match tool {
        "git" => Some("cgr.dev/chainguard/git:latest".into()),
        "curl" => Some("cgr.dev/chainguard/curl:latest".into()),
        "python" => Some("cgr.dev/chainguard/python:latest".into()),
        _ => None,
    }
}
