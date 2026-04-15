//! Image signature verification using Sigstore/cosign.
//!
//! Provides cryptographic verification of OCI images before execution.
//! Uses the `cosign` CLI for verification and `crane` / backend CLI
//! for digest resolution.

use super::types::ContainerError;
use std::collections::HashMap;
use std::sync::{RwLock, OnceLock};
use std::time::{Duration, Instant};
use tokio::process::Command;

/// Verification cache entry.
struct CacheEntry {
    verified: bool,
    timestamp: Instant,
    reason: Option<String>,
}

/// Global verification cache, keyed by image digest.
static VERIFICATION_CACHE: OnceLock<RwLock<HashMap<String, CacheEntry>>> = OnceLock::new();

/// Chainguard signing identity for certificate validation.
const CHAINGUARD_IDENTITY: &str =
    "https://github.com/chainguard-images/images/.github/workflows/sign.yaml@refs/heads/main";
const CHAINGUARD_ISSUER: &str = "https://token.actions.githubusercontent.com";

/// Cache TTL: 1 hour.
const CACHE_TTL: Duration = Duration::from_secs(3600);

// ============ Public API ============

/// Verify an image reference using Sigstore/cosign.
///
/// Returns the verified digest on success, or a `ContainerError::VerificationFailed`
/// if the image cannot be verified. Results are cached by digest for `CACHE_TTL`.
pub async fn verify_image(reference: &str) -> Result<String, ContainerError> {
    // 1. Resolve to a digest (cache key)
    let digest = fetch_image_digest(reference).await?;

    // 2. Check cache
    let cache = VERIFICATION_CACHE.get_or_init(|| RwLock::new(HashMap::new()));
    {
        let rd = cache.read().unwrap();
        if let Some(entry) = rd.get(&digest) {
            if entry.timestamp.elapsed() < CACHE_TTL {
                return if entry.verified {
                    Ok(digest.clone())
                } else {
                    Err(ContainerError::VerificationFailed {
                        image: reference.to_string(),
                        reason: entry
                            .reason
                            .clone()
                            .unwrap_or_else(|| "cached verification failed".to_string()),
                    })
                };
            }
        }
    }

    // 3. Perform verification
    let result = perform_cosign_verify(reference, &digest).await;

    // 4. Update cache
    {
        let mut wr = cache.write().unwrap();
        match &result {
            Ok(_) => wr.insert(
                digest.clone(),
                CacheEntry {
                    verified: true,
                    timestamp: Instant::now(),
                    reason: None,
                },
            ),
            Err(e) => wr.insert(
                digest.clone(),
                CacheEntry {
                    verified: false,
                    timestamp: Instant::now(),
                    reason: Some(e.to_string()),
                },
            ),
        };
    }

    result.map(|_| digest)
}

/// Verify an image using a specific public key (keyful verification).
///
/// This is useful for images signed with specific keys rather than
/// keyless Fulcio certificates.
pub async fn verify_image_with_key(
    reference: &str,
    key_path: &str,
) -> Result<String, ContainerError> {
    let digest = fetch_image_digest(reference).await?;
    let cache = VERIFICATION_CACHE.get_or_init(|| RwLock::new(HashMap::new()));

    // Check cache
    {
        let rd = cache.read().unwrap();
        if let Some(entry) = rd.get(&digest) {
            if entry.timestamp.elapsed() < CACHE_TTL && entry.verified {
                return Ok(digest.clone());
            }
        }
    }

    // cosign verify --key <path> <reference>
    let output = Command::new("cosign")
        .args([
            "verify",
            "--key",
            key_path,
            "--output",
            "text",
            reference,
        ])
        .output()
        .await;

    match output {
        Ok(out) if out.status.success() => {
            let mut wr = cache.write().unwrap();
            wr.insert(
                digest.clone(),
                CacheEntry {
                    verified: true,
                    timestamp: Instant::now(),
                    reason: None,
                },
            );
            Ok(digest)
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            let mut wr = cache.write().unwrap();
            wr.insert(
                digest.clone(),
                CacheEntry {
                    verified: false,
                    timestamp: Instant::now(),
                    reason: Some(stderr.clone()),
                },
            );
            Err(ContainerError::VerificationFailed {
                image: reference.to_string(),
                reason: stderr,
            })
        }
        Err(e) => {
            // cosign not found — not an error, just unverified
            Err(ContainerError::VerificationFailed {
                image: reference.to_string(),
                reason: format!("cosign binary not found: {}", e),
            })
        }
    }
}

// ============ Digest resolution ============

/// Fetch image digest from the container runtime.
///
/// Tries `crane digest` first (more reliable for registry lookups),
/// then falls back to `docker manifest inspect` or `podman manifest inspect`.
async fn fetch_image_digest(reference: &str) -> Result<String, ContainerError> {
    // Try `crane digest`
    if let Ok(output) = Command::new("crane").args(["digest", reference]).output().await {
        if output.status.success() {
            let digest = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !digest.is_empty() {
                return Ok(digest);
            }
        }
    }

    // Try `docker manifest inspect` and extract digest
    if let Ok(output) = Command::new("docker")
        .args(["manifest", "inspect", reference])
        .output()
        .await
    {
        if output.status.success() {
            let json: serde_json::Value =
                serde_json::from_slice(&output.stdout).unwrap_or_default();
            if let Some(digest) = json
                .get("manifest")
                .and_then(|m| m.get("digest"))
                .and_then(|d| d.as_str())
            {
                return Ok(digest.to_string());
            }
            // Fallback: config digest
            if let Some(digest) = json
                .get("manifest")
                .and_then(|m| m.get("config"))
                .and_then(|c| c.get("digest"))
                .and_then(|d| d.as_str())
            {
                return Ok(digest.to_string());
            }
        }
    }

    // Try `podman manifest inspect`
    if let Ok(output) = Command::new("podman")
        .args(["manifest", "inspect", reference])
        .output()
        .await
    {
        if output.status.success() {
            let json: serde_json::Value =
                serde_json::from_slice(&output.stdout).unwrap_or_default();
            if let Some(digest) = json.get("digest").and_then(|d| d.as_str()) {
                return Ok(digest.to_string());
            }
        }
    }

    // If all methods fail, return an error. PRODUCTION READINESS: never fall back to unverified tag.
    Err(ContainerError::NotFound(format!("Could not resolve digest for image: {}", reference)))
}

// ============ Cosign verification ============

/// Perform keyless cosign verification against Chainguard's identity.
///
/// Uses `cosign verify --certificate-identity` and `--certificate-oidc-issuer`
/// for keyless verification, then falls back to basic verification.
async fn perform_cosign_verify(
    reference: &str,
    _digest: &str,
) -> Result<(), ContainerError> {
    // 1. Try keyless verification with Chainguard identity
    let keyless_result = Command::new("cosign")
        .args([
            "verify",
            "--certificate-identity",
            CHAINGUARD_IDENTITY,
            "--certificate-oidc-issuer",
            CHAINGUARD_ISSUER,
            "--output",
            "text",
            reference,
        ])
        .output()
        .await;

    match keyless_result {
        Ok(out) if out.status.success() => Ok(()),
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            // If keyless fails with "no matching signatures", try basic verify
            if stderr.contains("no matching signatures") || stderr.contains("no signatures found")
            {
                return perform_basic_verify(reference).await;
            }

            Err(ContainerError::VerificationFailed {
                image: reference.to_string(),
                reason: format!("cosign verification failed: {}", stderr),
            })
        }
        Err(e) => {
            Err(ContainerError::VerificationFailed {
                image: reference.to_string(),
                reason: format!("cosign execution failed (is cosign installed?): {}", e),
            })
        }
    }
}

/// Basic cosign verification (without keyless identity check).
async fn perform_basic_verify(reference: &str) -> Result<(), ContainerError> {
    let output = Command::new("cosign")
        .args(["verify", "--output", "text", reference])
        .output()
        .await;

    match output {
        Ok(out) if out.status.success() => Ok(()),
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            Err(ContainerError::VerificationFailed {
                image: reference.to_string(),
                reason: format!("basic cosign verification failed: {}", stderr),
            })
        }
        Err(e) => Err(ContainerError::VerificationFailed {
            image: reference.to_string(),
            reason: format!("cosign execution failed (is cosign installed?): {}", e),
        }),
    }
}

// ============ Chainguard image lookup ============

/// Comprehensive lookup table mapping common tool names to Chainguard images.
///
/// Chainguard Images are maintained by Chainguard and are signed/verified
/// with Sigstore cosign. See <https://images.chainguard.dev/>.
pub fn get_chainguard_image(tool: &str) -> Option<String> {
    match tool {
        // Build tools
        "make" => Some("cgr.dev/chainguard/make".to_string()),
        "cmake" => Some("cgr.dev/chainguard/cmake".to_string()),
        "gcc" | "g++" | "cc" | "c++" => Some("cgr.dev/chainguard/gcc".to_string()),
        "clang" | "clang++" => Some("cgr.dev/chainguard/clang".to_string()),
        "rust" | "rustc" | "cargo" => Some("cgr.dev/chainguard/rust".to_string()),
        "go" | "golang" => Some("cgr.dev/chainguard/go".to_string()),
        "node" | "nodejs" | "npm" | "npx" => Some("cgr.dev/chainguard/node".to_string()),
        "python" | "python3" | "pip" | "pip3" => Some("cgr.dev/chainguard/python".to_string()),
        "ruby" | "gem" => Some("cgr.dev/chainguard/ruby".to_string()),
        "java" | "javac" | "jar" => Some("cgr.dev/chainguard/jdk".to_string()),
        "gradle" => Some("cgr.dev/chainguard/gradle".to_string()),
        "maven" => Some("cgr.dev/chainguard/maven".to_string()),

        // Network / HTTP
        "git" => Some("cgr.dev/chainguard/git".to_string()),
        "curl" => Some("cgr.dev/chainguard/curl".to_string()),
        "wget" => Some("cgr.dev/chainguard/wget".to_string()),
        "ssh" | "scp" | "sftp" => Some("cgr.dev/chainguard/openssh".to_string()),
        "openssl" => Some("cgr.dev/chainguard/openssl".to_string()) ,

        // Shell / coreutils
        "bash" => Some("cgr.dev/chainguard/bash".to_string()),
        "sh" | "ash" | "busybox" => Some("cgr.dev/chainguard/busybox".to_string()),
        "zsh" => Some("cgr.dev/chainguard/zsh".to_string()),
        "awk" | "gawk" => Some("cgr.dev/chainguard/gawk".to_string()),
        "sed" => Some("cgr.dev/chainguard/sed".to_string()),
        "grep" => Some("cgr.dev/chainguard/grep".to_string()),
        "jq" => Some("cgr.dev/chainguard/jq".to_string()),
        "yq" => Some("cgr.dev/chainguard/yq".to_string()),
        "tar" => Some("cgr.dev/chainguard/tar".to_string()),
        "zip" | "unzip" => Some("cgr.dev/chainguard/zip".to_string()),

        // Package managers
        "apt" | "apt-get" | "dpkg" => Some("cgr.dev/chainguard/wolfi-base".to_string()),
        "apk" => Some("cgr.dev/chainguard/wolfi-base".to_string()),
        "yum" | "dnf" | "rpm" => Some("cgr.dev/chainguard/wolfi-base".to_string()),

        // DevOps / cloud
        "docker" => Some("cgr.dev/chainguard/docker".to_string()),
        "kubectl" | "k8s" => Some("cgr.dev/chainguard/kubectl".to_string()),
        "helm" => Some("cgr.dev/chainguard/helm".to_string()),
        "terraform" => Some("cgr.dev/chainguard/terraform".to_string()),
        "aws" | "awscli" => Some("cgr.dev/chainguard/aws-cli".to_string()),
        "az" | "azure" => Some("cgr.dev/chainguard/azure-cli".to_string()),
        "gcloud" => Some("cgr.dev/chainguard/gcloud".to_string()),

        // Databases / caching
        "redis-cli" | "redis" => Some("cgr.dev/chainguard/redis".to_string()),
        "psql" | "postgres" => Some("cgr.dev/chainguard/postgres".to_string()),
        "mysql" | "mariadb" => Some("cgr.dev/chainguard/mariadb".to_string()),
        "sqlite3" | "sqlite" => Some("cgr.dev/chainguard/sqlite".to_string()),
        "mongosh" | "mongo" => Some("cgr.dev/chainguard/mongodb".to_string()),

        // Utilities
        "htop" | "top" => Some("cgr.dev/chainguard/procps".to_string()),
        "vim" | "vi" | "nvim" => Some("cgr.dev/chainguard/vim".to_string()),
        "nano" => Some("cgr.dev/chainguard/nano".to_string()),
        "less" | "more" => Some("cgr.dev/chainguard/less".to_string()),
        "file" => Some("cgr.dev/chainguard/file".to_string()),
        "strace" => Some("cgr.dev/chainguard/strace".to_string()),
        "lsof" => Some("cgr.dev/chainguard/lsof".to_string()),
        "netcat" | "nc" => Some("cgr.dev/chainguard/netcat".to_string()),
        "rsync" => Some("cgr.dev/chainguard/rsync".to_string()),
        "socat" => Some("cgr.dev/chainguard/socat".to_string()),
        "nginx" => Some("cgr.dev/chainguard/nginx".to_string()),
        "caddy" => Some("cgr.dev/chainguard/caddy".to_string()),

        _ => None,
    }
}

/// Get the default base image for sandboxed containers.
pub fn get_default_base_image() -> String {
    "cgr.dev/chainguard/alpine-base".to_string()
}

/// Get a minimal static base image (for capability-style sandboxing).
pub fn get_static_base_image() -> String {
    "cgr.dev/chainguard/wolfi-base".to_string()
}

/// Clear the verification cache (useful for testing).
pub fn clear_verification_cache() {
    if let Some(cache) = VERIFICATION_CACHE.get() {
        let mut wr = cache.write().unwrap();
        wr.clear();
    }
}
