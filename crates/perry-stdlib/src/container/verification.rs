use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
use super::types::ComposeError;
use super::get_global_backend_instance;

pub const CHAINGUARD_IDENTITY: &str =
    "https://github.com/chainguard-images/images/.github/workflows/sign.yaml@refs/heads/main";
pub const CHAINGUARD_ISSUER: &str =
    "https://token.actions.githubusercontent.com";

#[derive(Debug, Clone)]
enum VerificationResult {
    Verified,
    Failed(String),   // reason
}

static VERIFICATION_CACHE: OnceLock<RwLock<HashMap<String, VerificationResult>>> = OnceLock::new();

pub async fn verify_image(reference: &str) -> Result<String, ComposeError> {
    // 1. Fetch digest
    let digest = fetch_image_digest(reference).await?;

    // 2. Check cache
    let cache = VERIFICATION_CACHE.get_or_init(|| RwLock::new(HashMap::new()));
    {
        let read = cache.read().unwrap();
        if let Some(res) = read.get(&digest) {
            return match res {
                VerificationResult::Verified => Ok(digest.clone()),
                VerificationResult::Failed(r) => Err(ComposeError::VerificationFailed {
                    image: reference.to_string(),
                    reason: r.clone(),
                }),
            };
        }
    }

    // 3. Run cosign verify (simulation)
    let result = run_cosign_verify(reference, &digest).await;

    // 4. Cache result
    {
        let mut write = cache.write().unwrap();
        write.insert(digest.clone(), result.clone());
    }

    match result {
        VerificationResult::Verified => Ok(digest),
        VerificationResult::Failed(r) => Err(ComposeError::VerificationFailed {
            image: reference.to_string(),
            reason: r,
        }),
    }
}

async fn fetch_image_digest(reference: &str) -> Result<String, ComposeError> {
    if reference.contains('@') {
        return Ok(reference.split('@').last().unwrap().to_string());
    }

    let backend = get_global_backend_instance().await
        .map_err(|e| ComposeError::BackendNotAvailable { name: "global".into(), reason: e })?;

    let info = backend.inspect_image(reference).await?;
    if info.id.is_empty() {
        return Err(ComposeError::NotFound(format!("Image not found: {}", reference)));
    }
    Ok(info.id)
}

async fn run_cosign_verify(reference: &str, digest: &str) -> VerificationResult {
    let mut cmd = tokio::process::Command::new("cosign");
    cmd.args([
        "verify",
        "--certificate-identity", CHAINGUARD_IDENTITY,
        "--certificate-oidc-issuer", CHAINGUARD_ISSUER,
        &format!("{}@{}", reference, digest)
    ]);
    match cmd.output().await {
        Ok(output) if output.status.success() => VerificationResult::Verified,
        Ok(output) => VerificationResult::Failed(String::from_utf8_lossy(&output.stderr).to_string()),
        Err(e) => VerificationResult::Failed(e.to_string()),
    }
}

pub fn get_chainguard_image(tool: &str) -> Option<String> {
    match tool {
        "git" => Some("cgr.dev/chainguard/git".to_string()),
        "curl" => Some("cgr.dev/chainguard/curl".to_string()),
        "wget" => Some("cgr.dev/chainguard/wget".to_string()),
        "openssl" => Some("cgr.dev/chainguard/openssl".to_string()),
        "bash" => Some("cgr.dev/chainguard/bash".to_string()),
        "sh" => Some("cgr.dev/chainguard/busybox".to_string()),
        "node" => Some("cgr.dev/chainguard/node".to_string()),
        "python" => Some("cgr.dev/chainguard/python".to_string()),
        "ruby" => Some("cgr.dev/chainguard/ruby".to_string()),
        "go" => Some("cgr.dev/chainguard/go".to_string()),
        "rust" => Some("cgr.dev/chainguard/rust".to_string()),
        _ => None,
    }
}

pub fn get_default_base_image() -> &'static str {
    "cgr.dev/chainguard/alpine-base"
}
