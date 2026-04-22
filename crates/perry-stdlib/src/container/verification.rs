use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
use perry_container_compose::error::ComposeError;
use tokio::process::Command;
use std::process::Stdio;

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

pub fn get_default_base_image() -> &'static str {
    "cgr.dev/chainguard/alpine-base"
}

pub fn get_chainguard_image(tool: &str) -> String {
    match tool {
        "node" => "cgr.dev/chainguard/node:latest".to_string(),
        "python" => "cgr.dev/chainguard/python:latest".to_string(),
        "go" => "cgr.dev/chainguard/go:latest".to_string(),
        _ => get_default_base_image().to_string(),
    }
}

pub async fn verify_image(reference: &str) -> Result<String, ComposeError> {
    // 1. Resolve digest (simulation for now)
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

    // 3. Real cosign verify
    let output = Command::new("cosign")
        .args([
            "verify",
            "--certificate-identity", CHAINGUARD_IDENTITY,
            "--certificate-oidc-issuer", CHAINGUARD_ISSUER,
            reference,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await;

    let result = match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            // Parse digest from output (usually contains JSON with critical data)
            if let Some(caps) = regex::Regex::new(r"sha256:[a-f0-9]{64}").unwrap().find(&stdout) {
                VerificationResult::Verified(caps.as_str().to_string())
            } else {
                VerificationResult::Verified(digest.clone()) // Fallback to resolved digest
            }
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            VerificationResult::Failed(stderr.to_string())
        }
        Err(e) => {
            VerificationResult::Failed(e.to_string())
        }
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
