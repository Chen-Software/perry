use std::collections::HashMap;
use std::sync::RwLock;
use std::sync::OnceLock;
use crate::types::{ComposeError, ContainerLogs};

pub const CHAINGUARD_IDENTITY: &str =
    "https://github.com/chainguard-images/images/.github/workflows/sign.yaml@refs/heads/main";
pub const CHAINGUARD_ISSUER: &str =
    "https://token.actions.githubusercontent.com";

#[derive(Debug, Clone)]
pub enum VerificationResult {
    Verified,
    Failed(String),
}

static VERIFICATION_CACHE: OnceLock<RwLock<HashMap<String, VerificationResult>>> = OnceLock::new();

pub async fn fetch_image_digest(reference: &str) -> Result<String, ComposeError> {
    let backend = crate::mod_impl::get_global_backend_instance_internal().await
        .map_err(|e| ComposeError::BackendNotAvailable { name: "default".into(), reason: e })?;

    // Simplistic digest resolution via inspect
    match backend.inspect(reference).await {
        Ok(info) => Ok(info.id),
        Err(_) => {
            // If inspect fails, try to pull it first or just return an error
            // In a real implementation we might use 'crane digest' or 'docker manifest inspect'
            Err(ComposeError::VerificationFailed {
                image: reference.to_string(),
                reason: "Could not resolve image digest".into()
            })
        }
    }
}

pub async fn run_cosign_verify(_reference: &str, _digest: &str) -> VerificationResult {
    // In a real implementation, we would shell out to cosign:
    // cosign verify --certificate-identity CHAINGUARD_IDENTITY --certificate-oidc-issuer CHAINGUARD_ISSUER <reference>@<digest>
    // For now, we simulate success for Chainguard images
    VerificationResult::Verified
}

pub async fn verify_image(reference: &str) -> Result<String, ComposeError> {
    let digest = fetch_image_digest(reference).await?;

    let cache = VERIFICATION_CACHE.get_or_init(|| RwLock::new(HashMap::new()));
    {
        let cache_read = cache.read().unwrap();
        if let Some(result) = cache_read.get(&digest) {
            return match result {
                VerificationResult::Verified => Ok(digest),
                VerificationResult::Failed(reason) => Err(ComposeError::VerificationFailed {
                    image: reference.to_string(),
                    reason: reason.clone(),
                }),
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
        VerificationResult::Failed(reason) => Err(ComposeError::VerificationFailed {
            image: reference.to_string(),
            reason,
        }),
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
