//! Sigstore/cosign image verification for perry-container

use crate::container::types::{ComposeError, Result};
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
use tokio::process::Command;

pub const CHAINGUARD_IDENTITY: &str = "https://github.com/chainguard-images/images/.github/workflows/release.yaml@refs/heads/main";
pub const CHAINGUARD_ISSUER: &str = "https://token.actions.githubusercontent.com";

#[derive(Debug, Clone)]
pub struct VerificationResult {
    pub digest: String,
    pub verified: bool,
    pub reason: String,
}

static VERIFICATION_CACHE: OnceLock<RwLock<HashMap<String, VerificationResult>>> = OnceLock::new();

fn cache() -> &'static RwLock<HashMap<String, VerificationResult>> {
    VERIFICATION_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

pub async fn fetch_image_digest(reference: &str) -> Result<String> {
    // Attempt crane digest first (standard for Chainguard images)
    if let Ok(output) = Command::new("crane").args(["digest", reference]).output().await {
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
        }
    }

    // Fallback to docker/podman manifest inspect
    let bin = if which::which("podman").is_ok() { "podman" } else { "docker" };
    let output = Command::new(bin).args(["manifest", "inspect", reference]).output().await
        .map_err(ComposeError::IoError)?;

    if output.status.success() {
        let val: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(ComposeError::JsonError)?;
        if let Some(digest) = val["config"]["digest"].as_str() {
            return Ok(digest.to_string());
        }
        if let Some(digest) = val["manifests"][0]["digest"].as_str() {
             return Ok(digest.to_string());
        }
    }

    Err(ComposeError::VerificationFailed {
        image: reference.into(),
        reason: "failed to fetch image digest".into()
    })
}

pub async fn run_cosign_verify(reference: &str, digest: &str) -> VerificationResult {
    let image_with_digest = format!("{}@{}", reference.split('@').next().unwrap(), digest);

    let output = Command::new("cosign")
        .args([
            "verify",
            "--certificate-identity", CHAINGUARD_IDENTITY,
            "--certificate-oidc-issuer", CHAINGUARD_ISSUER,
            &image_with_digest
        ])
        .output()
        .await;

    match output {
        Ok(out) if out.status.success() => VerificationResult {
            digest: digest.to_string(),
            verified: true,
            reason: "verified via sigstore (Chainguard)".into(),
        },
        Ok(out) => VerificationResult {
            digest: digest.to_string(),
            verified: false,
            reason: String::from_utf8_lossy(&out.stderr).trim().to_string(),
        },
        Err(e) => VerificationResult {
            digest: digest.to_string(),
            verified: false,
            reason: format!("failed to run cosign: {}", e),
        },
    }
}

pub async fn verify_image(reference: &str) -> Result<String> {
    let digest = fetch_image_digest(reference).await?;

    {
        let read = cache().read().unwrap();
        if let Some(res) = read.get(&digest) {
            if res.verified {
                return Ok(digest);
            } else {
                return Err(ComposeError::VerificationFailed { image: reference.into(), reason: res.reason.clone() });
            }
        }
    }

    let res = run_cosign_verify(reference, &digest).await;
    let mut write = cache().write().unwrap();
    write.insert(digest.clone(), res.clone());

    if res.verified {
        Ok(digest)
    } else {
        Err(ComposeError::VerificationFailed { image: reference.into(), reason: res.reason })
    }
}

pub fn get_default_base_image() -> &'static str {
    "cgr.dev/chainguard/alpine-base:latest"
}

pub fn get_chainguard_image(tool: &str) -> Option<String> {
    match tool {
        "node" => Some("cgr.dev/chainguard/node:latest".into()),
        "python" => Some("cgr.dev/chainguard/python:latest".into()),
        "go" => Some("cgr.dev/chainguard/go:latest".into()),
        "rust" | "cargo" => Some("cgr.dev/chainguard/rust:latest".into()),
        _ => None,
    }
}
