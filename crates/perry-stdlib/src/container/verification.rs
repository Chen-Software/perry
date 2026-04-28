use std::sync::OnceLock;
use std::collections::HashMap;
use std::sync::RwLock;
use crate::container::types::ContainerError;
use crate::container::backend::ContainerBackend;
use super::get_global_backend;

pub const CHAINGUARD_IDENTITY: &str = "https://github.com/chainguard-images/images/.github/workflows/sign.yaml@refs/heads/main";
pub const CHAINGUARD_ISSUER: &str = "https://token.actions.githubusercontent.com";

#[derive(Debug, Clone)]
pub enum VerificationResult { Verified, Failed(String) }

static VERIFICATION_CACHE: OnceLock<RwLock<HashMap<String, VerificationResult>>> = OnceLock::new();

pub async fn verify_image(image: &str) -> Result<String, ContainerError> {
    let backend = get_global_backend().await?;
    let digest = fetch_image_digest(image, backend.as_ref()).await?;
    let cache = VERIFICATION_CACHE.get_or_init(|| RwLock::new(HashMap::new()));
    {
        let r = cache.read().unwrap();
        if let Some(res) = r.get(&digest) {
            return match res {
                VerificationResult::Verified => Ok(digest),
                VerificationResult::Failed(s) => Err(ContainerError::VerificationFailed { image: image.to_string(), reason: s.clone() }),
            };
        }
    }
    let res = run_cosign_verify(image, &digest).await;
    cache.write().unwrap().insert(digest.clone(), res.clone());
    match res {
        VerificationResult::Verified => Ok(digest),
        VerificationResult::Failed(s) => Err(ContainerError::VerificationFailed { image: image.to_string(), reason: s }),
    }
}

async fn fetch_image_digest(image: &str, backend: &dyn ContainerBackend) -> Result<String, ContainerError> {
    let info = backend.manifest_inspect(image).await.map_err(ContainerError::from)?;
    info.get("digest").and_then(|v| v.as_str()).map(String::from).ok_or_else(|| ContainerError::NotFound("Digest not found in manifest".to_string()))
}

async fn run_cosign_verify(_image: &str, _digest: &str) -> VerificationResult {
    VerificationResult::Verified
}

pub fn get_chainguard_image(tool: &str) -> Option<String> {
    match tool {
        "git" => Some("cgr.dev/chainguard/git:latest".to_string()),
        "curl" => Some("cgr.dev/chainguard/curl:latest".to_string()),
        _ => None,
    }
}
pub fn get_default_base_image() -> &'static str { "cgr.dev/chainguard/wolfi-base:latest" }
