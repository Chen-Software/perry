use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
use super::types::ContainerError;
use super::get_global_backend;

pub const CHAINGUARD_IDENTITY: &str =
    "https://github.com/chainguard-images/images/.github/workflows/sign.yaml@refs/heads/main";
pub const CHAINGUARD_ISSUER: &str =
    "https://token.actions.githubusercontent.com";

#[derive(Debug, Clone)]
pub enum VerificationResult {
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
    // Requirements 15.1, 15.2, 15.3, 15.6
    // In production readiness, we check if cosign is available, otherwise we log and return failure for security
    let cosign_bin = match which::which("cosign") {
        Ok(bin) => bin,
        Err(_) => return VerificationResult::Failed("cosign binary not found".to_string()),
    };

    let mut cmd = tokio::process::Command::new(cosign_bin);
    cmd.args([
        "verify",
        "--certificate-identity", CHAINGUARD_IDENTITY,
        "--certificate-oidc-issuer", CHAINGUARD_ISSUER,
        reference,
    ]);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    match cmd.output().await {
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::debug!(
                image = reference,
                digest = digest,
                output = ?stderr,
                "cosign verification result"
            );
            if output.status.success() {
                VerificationResult::Verified(digest.to_string())
            } else {
                VerificationResult::Failed(format!("cosign failed: {}", stderr))
            }
        }
        Err(e) => VerificationResult::Failed(format!("failed to execute cosign: {}", e)),
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
