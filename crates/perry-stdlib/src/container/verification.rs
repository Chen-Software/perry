//! Sigstore/cosign verification for OCI images

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
use perry_container_compose::error::ComposeError as ContainerError;

pub const CHAINGUARD_IDENTITY: &str =
    "https://github.com/chainguard-images/images/.github/workflows/sign.yaml@refs/heads/main";
pub const CHAINGUARD_ISSUER: &str =
    "https://token.actions.githubusercontent.com";

#[derive(Clone, Debug)]
pub enum VerificationResult {
    Verified,
    Failed(String),
}

static VERIFICATION_CACHE: OnceLock<RwLock<HashMap<String, VerificationResult>>> = OnceLock::new();

pub async fn verify_image(reference: &str) -> Result<String, ContainerError> {
    // 1. Fetch digest (tag -> digest resolution)
    // Requirement 15.4: fetches digest
    let digest = fetch_image_digest(reference).await?;

    // 2. Check cache (keyed by digest, not tag)
    // Requirement 15.4: caches result by digest
    let cache = VERIFICATION_CACHE.get_or_init(|| RwLock::new(HashMap::new()));
    {
        let read = cache.read().unwrap();
        if let Some(res) = read.get(&digest) {
            match res {
                VerificationResult::Verified => return Ok(digest),
                VerificationResult::Failed(reason) => return Err(ContainerError::VerificationFailed {
                    image: reference.to_string(),
                    reason: reason.clone()
                }),
            }
        }
    }

    // 3. Run cosign verify
    // Requirement 15.1, 15.2, 15.3
    let result = run_cosign_verify(reference, &digest).await;

    // 4. Cache and return
    {
        let mut write = cache.write().unwrap();
        write.insert(digest.clone(), result.clone());
    }

    match result {
        VerificationResult::Verified => Ok(digest),
        VerificationResult::Failed(reason) => Err(ContainerError::VerificationFailed {
            image: reference.to_string(),
            reason
        }),
    }
}

async fn fetch_image_digest(reference: &str) -> Result<String, ContainerError> {
    // Requirement 15.4: fetches digest
    let backend = crate::container::get_global_backend_instance_async().await.map_err(|e| ContainerError::BackendError {
        code: 1,
        message: e
    })?;

    let info = backend.inspect_image(reference).await?;
    Ok(info.id) // Usually info.id is the digest in OCI backends
}

async fn run_cosign_verify(reference: &str, _digest: &str) -> VerificationResult {
    // Requirement 15.3: shells out to cosign verify
    use tokio::process::Command;

    let mut cmd = Command::new("cosign");
    cmd.args([
        "verify",
        "--certificate-identity", CHAINGUARD_IDENTITY,
        "--certificate-oidc-issuer", CHAINGUARD_ISSUER,
        reference
    ]);

    match cmd.output().await {
        Ok(output) if output.status.success() => VerificationResult::Verified,
        Ok(output) => VerificationResult::Failed(String::from_utf8_lossy(&output.stderr).to_string()),
        Err(e) => VerificationResult::Failed(format!("cosign binary not found or failed: {}", e)),
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_verification_cache_idempotence() {
        let img = "cgr.dev/chainguard/alpine-base";
        let res1 = verify_image(img).await;
        let res2 = verify_image(img).await;

        match (res1, res2) {
            (Ok(d1), Ok(d2)) => assert_eq!(d1, d2),
            (Err(e1), Err(e2)) => assert_eq!(e1.to_string(), e2.to_string()),
            _ => panic!("Non-idempotent result for image verification"),
        }
    }
}
