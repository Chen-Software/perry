use crate::container::types::ContainerError;
use std::sync::{OnceLock, Mutex};
use std::collections::HashMap;
use tokio::process::Command;

static VERIFICATION_CACHE: OnceLock<Mutex<HashMap<String, Result<String, String>>>> = OnceLock::new();

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

    {
        let guard = cache.lock().unwrap();
        if let Some(res) = guard.get(reference) {
            return res.clone().map_err(|e| ContainerError::VerificationFailed {
                image: reference.to_string(),
                reason: e
            });
        }
    }

    // Call cosign CLI
    let output = Command::new("cosign")
        .args(&["verify", "--certificate-identity", "https://github.com/chainguard-images/images/.github/workflows/release.yaml@refs/heads/main", "--certificate-oidc-issuer", "https://token.actions.githubusercontent.com", reference])
        .output()
        .await;

    let res = match output {
        Ok(out) if out.status.success() => {
            // Extract digest from output (simplified)
            Ok(reference.to_string())
        }
        Ok(out) => Err(String::from_utf8_lossy(&out.stderr).to_string()),
        Err(e) => Err(e.to_string()),
    };

    let mut guard = cache.lock().unwrap();
    guard.insert(reference.to_string(), res.clone());

    res.map_err(|e| ContainerError::VerificationFailed { image: reference.to_string(), reason: e })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chainguard_constants() {
        assert_eq!(get_default_base_image(), "cgr.dev/chainguard/alpine-base");
    }
}
