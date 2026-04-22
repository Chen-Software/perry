use crate::container::types::*;
use crate::container::mod_utils::get_global_backend_instance;
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
use tokio::process::Command;
use tokio::time::{timeout, Duration};

pub const CHAINGUARD_IDENTITY: &str =
    "https://github.com/chainguard-images/images/.github/workflows/sign.yaml@refs/heads/main";
pub const CHAINGUARD_ISSUER: &str =
    "https://token.actions.githubusercontent.com";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationResult {
    Verified(String), // returns digest
    Failed(String),
}

static VERIFICATION_CACHE: OnceLock<RwLock<HashMap<String, VerificationResult>>> = OnceLock::new();

pub async fn verify_image(reference: &str) -> Result<String, String> {
    // 1. Fetch digest
    let digest = fetch_image_digest(reference).await?;

    // 2. Check cache
    let cache = VERIFICATION_CACHE.get_or_init(|| RwLock::new(HashMap::new()));
    {
        let r = cache.read().unwrap();
        if let Some(res) = r.get(&digest) {
            return match res {
                VerificationResult::Verified(d) => Ok(d.clone()),
                VerificationResult::Failed(reason) => Err(reason.clone()),
            };
        }
    }

    // 3. Run cosign verify (keyless Sigstore verification)
    let result = run_cosign_verify(reference, &digest).await;

    // 4. Cache result
    {
        let mut w = cache.write().unwrap();
        w.insert(digest.clone(), result.clone());
    }

    match result {
        VerificationResult::Verified(d) => Ok(d),
        VerificationResult::Failed(reason) => Err(reason),
    }
}

async fn fetch_image_digest(reference: &str) -> Result<String, String> {
    let backend = get_global_backend_instance().await?;
    match backend.inspect(reference).await {
        Ok(info) => {
             if info.id.starts_with("sha256:") {
                 Ok(info.id)
             } else {
                 // Try to find digest in info? For now assume Id is the digest if it looks like one
                 Ok(info.id)
             }
        }
        Err(e) => Err(format!("Failed to fetch digest for {}: {}", reference, e)),
    }
}

async fn run_cosign_verify(reference: &str, digest: &str) -> VerificationResult {
    // We check if 'cosign' is available. If not, we fail verification for safety
    // if it's a production-like requirement, but for this implementation
    // we'll attempt to run it.
    let cosign_bin = match perry_container_compose::backend::which_helper("cosign") {
        Ok(p) => p,
        Err(_) => return VerificationResult::Failed("cosign binary not found".into()),
    };

    let mut cmd = Command::new(cosign_bin);
    cmd.args([
        "verify",
        "--certificate-identity", CHAINGUARD_IDENTITY,
        "--certificate-oidc-issuer", CHAINGUARD_ISSUER,
        reference
    ]);

    let res = timeout(Duration::from_secs(10), cmd.output()).await;
    match res {
        Ok(Ok(output)) if output.status.success() => {
            VerificationResult::Verified(digest.to_string())
        }
        Ok(Ok(output)) => {
            VerificationResult::Failed(format!("cosign failed: {}", String::from_utf8_lossy(&output.stderr).trim()))
        }
        Ok(Err(e)) => {
            VerificationResult::Failed(format!("Failed to execute cosign: {}", e))
        }
        Err(_) => {
            VerificationResult::Failed("cosign verification timed out".into())
        }
    }
}

pub fn get_chainguard_image(tool: &str) -> Option<String> {
    match tool {
        "git" => Some("cgr.dev/chainguard/git".to_string()),
        "curl" => Some("cgr.dev/chainguard/curl".to_string()),
        "bash" => Some("cgr.dev/chainguard/bash".to_string()),
        _ => None,
    }
}

pub fn get_default_base_image() -> &'static str {
    "cgr.dev/chainguard/alpine-base"
}
