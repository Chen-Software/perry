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

    // 3. Perform cosign verify via backend callout per Requirement 15
    let result = if reference.starts_with("cgr.dev/chainguard/") {
        // Mocking the successful verification for chainguard images if cosign is not present,
        // but adding real shell logic for production readiness.
        let mut cmd = tokio::process::Command::new("cosign");
        cmd.args([
            "verify",
            "--certificate-identity", CHAINGUARD_IDENTITY,
            "--certificate-oidc-issuer", CHAINGUARD_ISSUER,
            reference
        ]);

        match cmd.output().await {
            Ok(output) if output.status.success() => {
                VerificationResult::Verified(digest.clone())
            }
            Ok(output) => {
                VerificationResult::Failed(String::from_utf8_lossy(&output.stderr).to_string())
            }
            Err(_) => {
                // If cosign is missing, we fail secure for production readiness
                // unless we are in a testing environment.
                if std::env::var("PERRY_SKIP_IMAGE_VERIFY").is_ok() {
                    VerificationResult::Verified(digest.clone())
                } else {
                    VerificationResult::Failed("cosign binary not found in PATH".into())
                }
            }
        }
    } else {
        // Non-chainguard images are rejected for capability tasks per Requirement 15.5
        VerificationResult::Failed("Only cryptographically verified Chainguard images are permitted for capability tasks".into())
    };

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
