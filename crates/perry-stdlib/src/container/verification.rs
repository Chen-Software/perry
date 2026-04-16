//! Image signature verification using Sigstore/cosign.
//!
//! Provides cryptographic verification of OCI images before execution.
//! Uses the `cosign` CLI for verification.

use super::types::ContainerError;
use std::collections::HashMap;
use std::sync::{RwLock, OnceLock};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationResult {
    Verified,
    Failed(String),
}

/// Global verification cache, keyed by image digest.
static VERIFICATION_CACHE: OnceLock<RwLock<HashMap<String, VerificationResult>>> = OnceLock::new();

/// Chainguard signing identity for certificate validation.
pub const CHAINGUARD_IDENTITY: &str =
    "https://github.com/chainguard-images/images/.github/workflows/sign.yaml@refs/heads/main";
pub const CHAINGUARD_ISSUER: &str =
    "https://token.actions.githubusercontent.com";

// ============ Public API ============

/// Verify an OCI image via Sigstore/cosign keyless verification.
/// Returns the image digest on success.
/// Results are cached by digest for the process lifetime.
pub async fn verify_image(reference: &str) -> Result<String, ContainerError> {
    // 1. Fetch digest (tag → digest resolution)
    let digest = fetch_image_digest(reference).await?;

    // 2. Check cache (keyed by digest, not tag)
    let cache = VERIFICATION_CACHE.get_or_init(|| RwLock::new(HashMap::new()));
    {
        let cache_read = cache.read().unwrap();
        if let Some(result) = cache_read.get(&digest) {
            return match result {
                VerificationResult::Verified => Ok(digest),
                VerificationResult::Failed(reason) => Err(ContainerError::VerificationFailed {
                    image: reference.to_string(),
                    reason: reason.clone(),
                }),
            };
        }
    }

    // 3. Run cosign verify (keyless, Sigstore public good instance)
    let result = run_cosign_verify(reference, &digest).await;

    // 4. Cache result
    {
        let mut cache_write = cache.write().unwrap();
        cache_write.insert(digest.clone(), result.clone());
    }

    match result {
        VerificationResult::Verified => Ok(digest),
        VerificationResult::Failed(reason) => Err(ContainerError::VerificationFailed {
            image: reference.to_string(),
            reason,
        }),
    }
}

// ============ Digest resolution ============

async fn fetch_image_digest(reference: &str) -> Result<String, ContainerError> {
    // In a real implementation, we would call the backend to get the digest.
    // For now, we return a mock digest if none is provided in the reference.
    if let Some(pos) = reference.find('@') {
        return Ok(reference[pos+1..].to_string());
    }

    // Fallback: use a stable hash of the reference as a mock digest
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    reference.hash(&mut hasher);
    Ok(format!("sha256:{:064x}", hasher.finish()))
}

// ============ Cosign verification ============

async fn run_cosign_verify(reference: &str, digest: &str) -> VerificationResult {
    use tokio::process::Command;
    use std::process::Stdio;

    let output = match Command::new("cosign")
        .arg("verify")
        .arg("--certificate-identity")
        .arg(CHAINGUARD_IDENTITY)
        .arg("--certificate-oidc-issuer")
        .arg(CHAINGUARD_ISSUER)
        .arg(format!("{}@{}", reference, digest))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
    {
        Ok(o) => o,
        Err(e) => return VerificationResult::Failed(format!("Failed to execute cosign: {}", e)),
    };

    if output.status.success() {
        VerificationResult::Verified
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        VerificationResult::Failed(stderr.to_string())
    }
}
// ============ Chainguard image lookup ============

/// Chainguard image lookup table
pub fn get_chainguard_image(tool: &str) -> Option<String> {
    match tool {
        "git"     => Some("cgr.dev/chainguard/git".to_string()),
        "curl"    => Some("cgr.dev/chainguard/curl".to_string()),
        "wget"    => Some("cgr.dev/chainguard/wget".to_string()),
        "openssl" => Some("cgr.dev/chainguard/openssl".to_string()),
        "bash"    => Some("cgr.dev/chainguard/bash".to_string()),
        "sh"      => Some("cgr.dev/chainguard/busybox".to_string()),
        "node"    => Some("cgr.dev/chainguard/node".to_string()),
        "python"  => Some("cgr.dev/chainguard/python".to_string()),
        "ruby"    => Some("cgr.dev/chainguard/ruby".to_string()),
        "go"      => Some("cgr.dev/chainguard/go".to_string()),
        "rust"    => Some("cgr.dev/chainguard/rust".to_string()),
        _         => None,
    }
}

pub fn get_default_base_image() -> &'static str {
    "cgr.dev/chainguard/alpine-base"
}


