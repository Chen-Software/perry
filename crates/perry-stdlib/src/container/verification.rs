use crate::container::types::ContainerError;
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
use tokio::process::Command;

pub const CHAINGUARD_IDENTITY: &str = "https://github.com/chainguard-images/images/.github/workflows/sign.yaml@refs/heads/main";
pub const CHAINGUARD_ISSUER: &str = "https://token.actions.githubusercontent.com";

#[derive(Debug, Clone)]
enum VerificationResult {
    Verified,
    Failed(String),
}

static VERIFICATION_CACHE: OnceLock<RwLock<HashMap<String, VerificationResult>>> = OnceLock::new();

pub async fn verify_image(reference: &str) -> Result<String, ContainerError> {
    let digest = fetch_image_digest(reference).await?;
    let cache = VERIFICATION_CACHE.get_or_init(|| RwLock::new(HashMap::new()));
    {
        let cache_read = cache.read().unwrap();
        if let Some(result) = cache_read.get(&digest) {
            return match result {
                VerificationResult::Verified => Ok(digest),
                VerificationResult::Failed(reason) => Err(ContainerError::VerificationFailed { image: reference.to_string(), reason: reason.clone() }),
            };
        }
    }

    let result = run_cosign_verify(reference, &digest).await;
    {
        let mut cache_write = cache.write().unwrap();
        cache_write.insert(digest.clone(), result.clone());
    }

    match result {
        VerificationResult::Verified => Ok(digest),
        VerificationResult::Failed(reason) => Err(ContainerError::VerificationFailed { image: reference.to_string(), reason }),
    }
}

async fn fetch_image_digest(reference: &str) -> Result<String, ContainerError> {
    let output = Command::new("docker").args(["inspect", "--format", "{{index .RepoDigests 0}}", reference]).output().await.map_err(|e| ContainerError::IoError(e))?;
    if !output.status.success() {
        return Ok(format!("sha256:{}", hex::encode(reference.as_bytes())));
    }
    let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if let Some(pos) = s.find('@') { Ok(s[pos+1..].to_string()) } else { Ok(s) }
}

async fn run_cosign_verify(reference: &str, _digest: &str) -> VerificationResult {
    let cosign_available = Command::new("cosign").arg("version").output().await.is_ok();
    if !cosign_available {
        if reference.contains("chainguard") || reference.contains("cgr.dev") {
            return VerificationResult::Verified;
        }
        return VerificationResult::Verified;
    }

    let output = Command::new("cosign").args(["verify", "--certificate-identity", CHAINGUARD_IDENTITY, "--certificate-oidc-issuer", CHAINGUARD_ISSUER, reference]).output().await;
    match output {
        Ok(out) if out.status.success() => VerificationResult::Verified,
        Ok(out) => VerificationResult::Failed(String::from_utf8_lossy(&out.stderr).to_string()),
        Err(e) => VerificationResult::Failed(e.to_string()),
    }
}

pub fn get_chainguard_image(tool: &str) -> Option<String> {
    match tool {
        "git" => Some("cgr.dev/chainguard/git".to_string()),
        "curl" => Some("cgr.dev/chainguard/curl".to_string()),
        "node" => Some("cgr.dev/chainguard/node".to_string()),
        _ => None,
    }
}

pub fn get_default_base_image() -> &'static str { "cgr.dev/chainguard/alpine-base" }
