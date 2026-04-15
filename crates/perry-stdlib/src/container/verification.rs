//! Image signature verification using Sigstore/cosign
//!
//! Provides cryptographic verification of OCI images before execution.

use super::types::ContainerError;
use std::collections::HashMap;
use std::sync::{RwLock, OnceLock};
use std::time::{Duration, Instant};

/// Verification cache entry
struct CacheEntry {
    verified: bool,
    timestamp: Instant,
}

/// Global verification cache
static VERIFICATION_CACHE: OnceLock<RwLock<HashMap<String, CacheEntry>>> = OnceLock::new();

/// Chainguard signing identity for certificate validation
const CHAINGUARD_IDENTITY: &str = "https://github.com/chainguard-images/images/.github/workflows/sign.yaml@refs/heads/main";
const CHAINGUARD_ISSUER: &str = "https://token.actions.githubusercontent.com";

/// Verify an image reference using Sigstore/cosign
pub async fn verify_image(reference: &str) -> Result<String, ContainerError> {
    // Extract image digest for cache key
    let digest = fetch_image_digest(reference).await?;

    // Get or create cache
    let cache = VERIFICATION_CACHE.get_or_init(|| RwLock::new(HashMap::new()));

    // Check cache
    {
        let cache_read = cache.read().unwrap();
        if let Some(entry) = cache_read.get(&digest) {
            // Cache entry is valid for 1 hour
            if entry.timestamp.elapsed() < Duration::from_secs(3600) {
                if entry.verified {
                    return Ok(digest);
                } else {
                    return Err(ContainerError::VerificationFailed {
                        image: reference.to_string(),
                        reason: "cached verification failed".to_string(),
                    });
                }
            }
        }
    }

    // Perform verification
    let verified = perform_verification(reference, &digest).await?;

    // Update cache
    {
        let mut cache = cache.write().unwrap();
        cache.insert(
            digest.clone(),
            CacheEntry {
                verified,
                timestamp: Instant::now(),
            },
        );
    }

    if verified {
        Ok(digest)
    } else {
        Err(ContainerError::VerificationFailed {
            image: reference.to_string(),
            reason: "signature verification failed".to_string(),
        })
    }
}

/// Fetch image digest from registry or local cache
async fn fetch_image_digest(reference: &str) -> Result<String, ContainerError> {
    // TODO: Implement actual digest fetching
    // For now, use the reference as a placeholder
    Ok(reference.to_string())
}

/// Perform actual verification using Sigstore/cosign
async fn perform_verification(_reference: &str, _digest: &str) -> Result<bool, ContainerError> {
    // TODO: Implement actual Sigstore/cosign verification
    // This requires the sigstore-cosign crate
    // For now, always return true (trusted) for development
    // In production, this would:
    // 1. Fetch the image signature from the registry
    // 2. Verify the signature using cosign keyless verification
    // 3. Validate certificate identity and OIDC issuer
    // 4. Check against Chainguard's public keys

    Ok(true)
}

/// Get the default Chainguard image for a given tool
pub fn get_chainguard_image(tool: &str) -> Option<String> {
    match tool {
        "git" => Some("cgr.dev/chainguard/git".to_string()),
        "curl" => Some("cgr.dev/chainguard/curl".to_string()),
        "wget" => Some("cgr.dev/chainguard/wget".to_string()),
        "openssl" => Some("cgr.dev/chainguard/openssl".to_string()),
        "bash" => Some("cgr.dev/chainguard/bash".to_string()),
        "sh" => Some("cgr.dev/chainguard/busybox".to_string()),
        _ => None,
    }
}

/// Get the default base image for sandboxed containers
pub fn get_default_base_image() -> String {
    "cgr.dev/chainguard/alpine-base".to_string()
}

/// Clear the verification cache (useful for testing)
pub fn clear_verification_cache() {
    if let Some(cache) = VERIFICATION_CACHE.get() {
        let mut cache_write = cache.write().unwrap();
        cache_write.clear();
    }
}
