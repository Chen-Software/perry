use crate::container::types::ContainerError;
use std::sync::{OnceLock, Mutex};
use std::collections::HashMap;
use tokio::process::Command;

pub const CHAINGUARD_IDENTITY: &str = "https://github.com/chainguard-images/images/.github/workflows/release.yaml@refs/heads/main";
pub const CHAINGUARD_ISSUER: &str = "https://token.actions.githubusercontent.com";

static VERIFICATION_CACHE: OnceLock<Mutex<HashMap<String, Result<String, String>>>> = OnceLock::new();

pub fn clear_verification_cache() {
    if let Some(cache) = VERIFICATION_CACHE.get() {
        cache.lock().unwrap().clear();
    }
}

pub fn get_default_base_image() -> &'static str {
    "cgr.dev/chainguard/alpine-base"
}

pub fn get_chainguard_image(tool: &str) -> Option<String> {
    match tool {
        "git" => Some("cgr.dev/chainguard/git".into()),
        "node" => Some("cgr.dev/chainguard/node".into()),
        "python" => Some("cgr.dev/chainguard/python".into()),
        _ => None,
    }
}

pub async fn verify_image(reference: &str) -> Result<String, ContainerError> {
    let cache = VERIFICATION_CACHE.get_or_init(|| Mutex::new(HashMap::new()));

    // Check if we have a verified digest for this reference
    {
        let guard = cache.lock().unwrap();
        if let Some(res) = guard.get(reference) {
            return res.clone().map_err(|e| ContainerError::VerificationFailed {
                image: reference.to_string(),
                reason: e
            });
        }
    }

    // Attempt to resolve digest first if it's a tag
    let digest = if reference.contains("@sha256:") {
        reference.split('@').nth(1).unwrap().to_string()
    } else {
        // Run cosign verify to get digest and verify signature
        let output = Command::new("cosign")
            .args(&["verify", "--certificate-identity", CHAINGUARD_IDENTITY, "--certificate-oidc-issuer", CHAINGUARD_ISSUER, reference])
            .output()
            .await
            .map_err(|e| ContainerError::VerificationFailed { image: reference.to_string(), reason: e.to_string() })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let mut guard = cache.lock().unwrap();
            guard.insert(reference.to_string(), Err(stderr.clone()));
            return Err(ContainerError::VerificationFailed { image: reference.to_string(), reason: stderr });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout.lines()
            .find(|l| l.contains("sha256:"))
            .and_then(|l| l.split("sha256:").nth(1))
            .map(|d| d.split_whitespace().next().unwrap_or(d).to_string())
            .ok_or_else(|| ContainerError::VerificationFailed {
                image: reference.to_string(),
                reason: "Could not find digest in verification output".to_string()
            })?
    };

    let mut guard = cache.lock().unwrap();
    guard.insert(reference.to_string(), Ok(digest.clone()));
    Ok(digest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chainguard_constants() {
        assert_eq!(get_default_base_image(), "cgr.dev/chainguard/alpine-base");
    }
}
