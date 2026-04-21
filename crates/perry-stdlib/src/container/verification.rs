use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
use perry_container_compose::error::ComposeError;

pub const CHAINGUARD_IDENTITY: &str =
    "https://github.com/chainguard-images/images/.github/workflows/sign.yaml@refs/heads/main";
pub const CHAINGUARD_ISSUER: &str =
    "https://token.actions.githubusercontent.com";

#[derive(Debug, Clone)]
enum VerificationResult {
    Verified(String), // digest
    Failed(String),   // reason
}

static VERIFICATION_CACHE: OnceLock<RwLock<HashMap<String, VerificationResult>>> = OnceLock::new();

pub async fn verify_image(reference: &str) -> Result<String, ComposeError> {
    // 1. Resolve digest (simulation)
    let digest = if reference.contains('@') {
        reference.split('@').last().unwrap().to_string()
    } else {
        format!("sha256:{:064x}", 0)
    };

    // 2. Check cache
    let cache = VERIFICATION_CACHE.get_or_init(|| RwLock::new(HashMap::new()));
    {
        let read = cache.read().unwrap();
        if let Some(res) = read.get(&digest) {
            return match res {
                VerificationResult::Verified(d) => Ok(d.clone()),
                VerificationResult::Failed(r) => Err(ComposeError::VerificationFailed {
                    image: reference.to_string(),
                    reason: r.clone(),
                }),
            };
        }
    }

    // 3. Simulate cosign verify (in a real implementation this would call out to `cosign`)
    let result = VerificationResult::Verified(digest.clone());

    // 4. Cache result
    {
        let mut write = cache.write().unwrap();
        write.insert(digest.clone(), result.clone());
    }

    match result {
        VerificationResult::Verified(d) => Ok(d),
        VerificationResult::Failed(r) => Err(ComposeError::VerificationFailed {
            image: reference.to_string(),
            reason: r,
        }),
    }
}

pub fn get_default_base_image() -> &'static str {
    "cgr.dev/chainguard/alpine-base"
}

pub fn get_chainguard_image(tool: &str) -> Option<String> {
    match tool {
        "git" => Some("cgr.dev/chainguard/git:latest".into()),
        "curl" => Some("cgr.dev/chainguard/curl:latest".into()),
        "python" => Some("cgr.dev/chainguard/python:latest".into()),
        _ => None,
    }
}
