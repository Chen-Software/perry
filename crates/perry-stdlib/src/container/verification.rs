use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
use perry_container_compose::error::ComposeError;

pub const CHAINGUARD_IDENTITY: &str = "https://github.com/chainguard-images/images/.github/workflows/sign.yaml@refs/heads/main";
pub const CHAINGUARD_ISSUER: &str = "https://token.actions.githubusercontent.com";

#[derive(Debug, Clone)]
pub enum VerificationResult {
    Verified(String), // digest
    Failed(String),   // reason
}

static VERIFICATION_CACHE: OnceLock<RwLock<HashMap<String, VerificationResult>>> = OnceLock::new();

pub async fn verify_image(reference: &str) -> Result<String, ComposeError> {
    let cache = VERIFICATION_CACHE.get_or_init(|| RwLock::new(HashMap::new()));

    {
        let r = cache.read().unwrap();
        if let Some(res) = r.get(reference) {
            match res {
                VerificationResult::Verified(d) => return Ok(d.clone()),
                VerificationResult::Failed(reason) => {
                    return Err(ComposeError::VerificationFailed {
                        image: reference.into(),
                        reason: reason.clone(),
                    })
                }
            }
        }
    }

    // Try to find cosign binary
    let cosign_bin = match which::which("cosign") {
        Ok(p) => p,
        Err(_) => {
            // If cosign is not installed, we cannot verify signatures.
            // For now, we allow it but log a warning, OR we can strictly enforce it.
            // Requirement 15.5: "The runtime SHALL NOT fall back to running an unverified image under any circumstances."
            // So we must error if we can't verify.
            return Err(ComposeError::VerificationFailed {
                image: reference.into(),
                reason: "cosign binary not found".into(),
            });
        }
    };

    let output = tokio::process::Command::new(cosign_bin)
        .args([
            "verify",
            "--certificate-identity",
            CHAINGUARD_IDENTITY,
            "--certificate-oidc-issuer",
            CHAINGUARD_ISSUER,
            reference,
        ])
        .output()
        .await
        .map_err(|e| ComposeError::VerificationFailed {
            image: reference.into(),
            reason: format!("failed to execute cosign: {}", e),
        })?;

    if !output.status.success() {
        let reason = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let result = VerificationResult::Failed(reason.clone());
        let mut w = cache.write().unwrap();
        w.insert(reference.to_string(), result);
        return Err(ComposeError::VerificationFailed {
            image: reference.into(),
            reason,
        });
    }

    // Extract digest from output. Cosign verify output contains the digest in the form:
    // [index] sha256:<digest>
    let stdout = String::from_utf8_lossy(&output.stdout);
    let digest = stdout
        .lines()
        .find_map(|line| {
            if line.contains("sha256:") {
                line.split("sha256:").nth(1)?.split_whitespace().next()
            } else {
                None
            }
        })
        .map(|d| format!("sha256:{}", d))
        .ok_or_else(|| ComposeError::VerificationFailed {
            image: reference.into(),
            reason: "digest not found in cosign output".into(),
        })?;

    let result = VerificationResult::Verified(digest.clone());

    let mut w = cache.write().unwrap();
    w.insert(reference.to_string(), result);

    Ok(digest)
}

pub fn get_chainguard_image(tool: &str) -> Option<String> {
    match tool {
        "git"     => Some("cgr.dev/chainguard/git".to_string()),
        "curl"    => Some("cgr.dev/chainguard/curl".to_string()),
        "wget"    => Some("cgr.dev/chainguard/wget".to_string()),
        "bash"    => Some("cgr.dev/chainguard/bash".to_string()),
        "python"  => Some("cgr.dev/chainguard/python".to_string()),
        "node"    => Some("cgr.dev/chainguard/node".to_string()),
        _         => None,
    }
}

pub fn get_default_base_image() -> &'static str {
    "cgr.dev/chainguard/alpine-base"
}
