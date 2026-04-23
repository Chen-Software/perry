use std::sync::{OnceLock, RwLock};
use std::collections::HashMap;
use std::process::Command;
use crate::container::error::ContainerError;

pub const CHAINGUARD_IDENTITY: &str = "https://github.com/chainguard-images/images/.github/workflows/release.yaml@refs/heads/main";
pub const CHAINGUARD_ISSUER: &str = "https://token.actions.githubusercontent.com";

/// Cache keyed by digest, stores whether it's verified.
static VERIFICATION_CACHE: OnceLock<RwLock<HashMap<String, bool>>> = OnceLock::new();

pub fn verify_image(reference: &str) -> Result<String, ContainerError> {
    // 1. Resolve digest
    let digest = fetch_digest(reference)?;
    let base_ref = reference.split('@').next().unwrap();
    let full_ref = format!("{}@{}", base_ref, digest);

    let cache = VERIFICATION_CACHE.get_or_init(|| RwLock::new(HashMap::new()));

    // 2. Check cache
    {
        let read_guard = cache.read().unwrap();
        if let Some(&verified) = read_guard.get(&digest) {
            if verified {
                return Ok(digest);
            } else {
                return Err(ContainerError::VerificationFailed {
                    image: reference.to_string(),
                    reason: "Cached verification failure".to_string()
                });
            }
        }
    }

    // 3. Verify with cosign
    let mut cmd = Command::new("cosign");
    cmd.args([
        "verify",
        "--certificate-identity", CHAINGUARD_IDENTITY,
        "--certificate-oidc-issuer", CHAINGUARD_ISSUER,
        &full_ref,
    ]);

    let output = cmd.output().map_err(|e| ContainerError::BackendError {
        code: 1,
        message: format!("Failed to execute cosign: {}", e)
    })?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Log verification attempt (using eprintln for visibility in dev)
    eprintln!("cosign verify output for {}:\nSTDOUT: {}\nSTDERR: {}", full_ref, stdout, stderr);

    if !output.status.success() {
        let mut write_guard = cache.write().unwrap();
        write_guard.insert(digest.clone(), false);
        return Err(ContainerError::VerificationFailed {
            image: reference.to_string(),
            reason: stderr.to_string()
        });
    }

    // 4. Update cache
    {
        let mut write_guard = cache.write().unwrap();
        write_guard.insert(digest.clone(), true);
    }

    Ok(digest)
}

fn fetch_digest(reference: &str) -> Result<String, ContainerError> {
    // If it already has a digest, return it
    if let Some((_, digest)) = reference.split_once('@') {
        return Ok(digest.to_string());
    }

    // Fallback 1: crane digest
    if let Ok(output) = Command::new("crane").args(["digest", reference]).output() {
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
        }
    }

    // Fallback 2: podman manifest inspect (works without pull)
    if let Ok(output) = Command::new("podman").args(["manifest", "inspect", reference]).output() {
        if output.status.success() {
            if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                if let Some(digest) = json["config"]["digest"].as_str() {
                    return Ok(digest.to_string());
                }
                // Also check manifest list
                if let Some(manifests) = json["manifests"].as_array() {
                    if let Some(first) = manifests.first() {
                         if let Some(digest) = first["digest"].as_str() {
                             return Ok(digest.to_string());
                         }
                    }
                }
            }
        }
    }

    // Fallback 3: docker manifest inspect (needs experimental features enabled often, but let's try)
    if let Ok(output) = Command::new("docker").args(["manifest", "inspect", reference]).output() {
        if output.status.success() {
            if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                // Docker manifest inspect returns a manifest list or a single manifest
                if let Some(digest) = json["config"]["digest"].as_str() {
                    return Ok(digest.to_string());
                }
                if let Some(manifests) = json["manifests"].as_array() {
                    if let Some(first) = manifests.first() {
                         if let Some(digest) = first["digest"].as_str() {
                             return Ok(digest.to_string());
                         }
                    }
                }
            }
        }
    }

    // Fallback 4: docker/podman inspect (if image is already pulled)
    for bin in ["docker", "podman"] {
        if let Ok(output) = Command::new(bin).args(["inspect", "--format", "{{index .RepoDigests 0}}", reference]).output() {
            if output.status.success() {
                let full = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if let Some((_, digest)) = full.split_once('@') {
                    return Ok(digest.to_string());
                }
            }
        }
    }

    Err(ContainerError::BackendError {
        code: 1,
        message: format!("Failed to resolve digest for image {}", reference),
    })
}

pub fn get_default_base_image() -> &'static str {
    "cgr.dev/chainguard/alpine-base"
}

pub fn get_chainguard_image(tool: &str) -> Option<String> {
    match tool {
        "node" => Some("cgr.dev/chainguard/node:latest".to_string()),
        "python" => Some("cgr.dev/chainguard/python:latest".to_string()),
        "git" => Some("cgr.dev/chainguard/git:latest".to_string()),
        "go" => Some("cgr.dev/chainguard/go:latest".to_string()),
        _ => None,
    }
}
