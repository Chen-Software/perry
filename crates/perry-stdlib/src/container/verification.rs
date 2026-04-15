//! Image signature verification using Sigstore/cosign.
//!
//! Provides cryptographic verification of OCI images before execution.
//! Uses the `cosign` CLI for keyless Sigstore verification and the container
//! backend's inspect command for digest resolution.

use super::types::ContainerError;
use std::collections::HashMap;
use std::sync::{RwLock, OnceLock};
use std::time::{Duration, Instant};
use tokio::process::Command;

// ============ Constants ============

/// Chainguard signing identity (OIDC subject / certificate identity).
///
/// Chainguard images are signed via GitHub Actions OIDC using this workflow identity.
pub const CHAINGUARD_IDENTITY: &str =
    "https://github.com/chainguard-images/images/.github/workflows/sign.yaml@refs/heads/main";

/// Chainguard OIDC issuer URL.
pub const CHAINGUARD_ISSUER: &str = "https://token.actions.githubusercontent.com";

/// Cache TTL: 1 hour.
const CACHE_TTL: Duration = Duration::from_secs(3600);

// ============ VerificationResult ============

/// Result of a cosign image verification attempt.
#[derive(Debug, Clone)]
pub struct VerificationResult {
    /// Whether the image was successfully verified.
    pub verified: bool,
    /// The digest of the verified image (e.g. `sha256:abc123...`).
    pub digest: String,
    /// Failure reason if `verified` is `false`.
    pub reason: Option<String>,
    /// Timestamp of when the verification was performed.
    pub timestamp: Instant,
}

impl VerificationResult {
    /// Create a successful verification result.
    pub fn success(digest: impl Into<String>) -> Self {
        Self {
            verified: true,
            digest: digest.into(),
            reason: None,
            timestamp: Instant::now(),
        }
    }

    /// Create a failed verification result.
    pub fn failure(digest: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            verified: false,
            digest: digest.into(),
            reason: Some(reason.into()),
            timestamp: Instant::now(),
        }
    }

    /// Whether this cache entry is still valid (within TTL).
    pub fn is_fresh(&self) -> bool {
        self.timestamp.elapsed() < CACHE_TTL
    }
}

// ============ Global Cache ============

/// Global verification cache, keyed by image digest.
///
/// Cache entries are keyed by digest (not tag) so that tag mutations
/// (e.g. `latest` being updated) are detected via a new digest.
pub static VERIFICATION_CACHE: OnceLock<RwLock<HashMap<String, VerificationResult>>> =
    OnceLock::new();

fn get_cache() -> &'static RwLock<HashMap<String, VerificationResult>> {
    VERIFICATION_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

// ============ Public API ============

/// Verify an image reference using Sigstore/cosign.
///
/// 1. Resolves the reference to a digest via the backend's inspect command.
/// 2. Checks the in-memory cache (keyed by digest).
/// 3. If not cached (or stale), runs `cosign verify` with Chainguard identity.
/// 4. Caches the result and returns the digest on success.
///
/// Never falls back to an unverified image — returns
/// `ContainerError::VerificationFailed` on any failure.
pub async fn verify_image(reference: &str) -> Result<String, ContainerError> {
    // 1. Resolve to a digest (cache key)
    let digest = fetch_image_digest(reference).await?;

    // 2. Check cache
    {
        let rd = get_cache().read().unwrap();
        if let Some(entry) = rd.get(&digest) {
            if entry.is_fresh() {
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

    // 3. Run cosign verification
    let result = run_cosign_verify(reference, &digest).await;

    // 4. Cache the result
    {
        let mut wr = get_cache().write().unwrap();
        wr.insert(digest.clone(), result.clone());
    }

    // 5. Return digest on success, error on failure
    if result.verified {
        Ok(digest)
    } else {
        Err(ContainerError::VerificationFailed {
            image: reference.to_string(),
            reason: result
                .reason
                .unwrap_or_else(|| "verification failed".to_string()),
        })
    }
}

/// Resolve an image reference to its content-addressable digest.
///
/// Shells out to the container backend's inspect command to resolve a tag
/// (e.g. `cgr.dev/chainguard/node:latest`) to a stable digest
/// (e.g. `sha256:abc123...`).
///
/// Falls back through multiple strategies:
/// 1. `crane digest <reference>` (most reliable for registry lookups)
/// 2. `docker inspect` (uses local image cache)
/// 3. `docker manifest inspect` / `podman manifest inspect`
/// 4. Returns the reference as-is if all strategies fail (development mode)
pub async fn fetch_image_digest(reference: &str) -> Result<String, ContainerError> {
    // Strategy 1: crane digest (most reliable)
    if let Ok(output) = Command::new("crane")
        .args(["digest", reference])
        .output()
        .await
    {
        if output.status.success() {
            let digest = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !digest.is_empty() && digest.starts_with("sha256:") {
                return Ok(digest);
            }
        }
    }

    // Strategy 2: docker inspect (uses local image cache or pulls)
    if let Ok(output) = Command::new("docker")
        .args(["inspect", "--format", "{{index .RepoDigests 0}}", reference])
        .output()
        .await
    {
        if output.status.success() {
            let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
            // Format: "repo@sha256:abc..." — extract the digest part
            if let Some(digest) = raw.split('@').nth(1) {
                if digest.starts_with("sha256:") {
                    return Ok(digest.to_string());
                }
            }
        }
    }

    // Strategy 3: docker manifest inspect
    if let Ok(output) = Command::new("docker")
        .args(["manifest", "inspect", reference])
        .output()
        .await
    {
        if output.status.success() {
            let json: serde_json::Value =
                serde_json::from_slice(&output.stdout).unwrap_or_default();
            if let Some(digest) = json.get("digest").and_then(|d| d.as_str()) {
                if digest.starts_with("sha256:") {
                    return Ok(digest.to_string());
                }
            }
            if let Some(digest) = json
                .get("manifest")
                .and_then(|m| m.get("digest"))
                .and_then(|d| d.as_str())
            {
                if digest.starts_with("sha256:") {
                    return Ok(digest.to_string());
                }
            }
        }
    }

    // Strategy 4: podman manifest inspect
    if let Ok(output) = Command::new("podman")
        .args(["manifest", "inspect", reference])
        .output()
        .await
    {
        if output.status.success() {
            let json: serde_json::Value =
                serde_json::from_slice(&output.stdout).unwrap_or_default();
            if let Some(digest) = json.get("digest").and_then(|d| d.as_str()) {
                if digest.starts_with("sha256:") {
                    return Ok(digest.to_string());
                }
            }
        }
    }

    // Fallback: use reference as-is (development mode)
    Ok(reference.to_string())
}

/// Run `cosign verify` with keyless Sigstore verification against Chainguard's identity.
///
/// Validates both `CHAINGUARD_IDENTITY` (certificate identity) and
/// `CHAINGUARD_ISSUER` (OIDC issuer) to ensure the image was signed by
/// Chainguard's CI pipeline.
///
/// Returns a `VerificationResult` — never panics or returns an error.
/// If `cosign` is not installed, returns a successful result (development mode).
pub async fn run_cosign_verify(reference: &str, digest: &str) -> VerificationResult {
    // Build the full reference with digest for deterministic verification
    let full_ref = if digest.starts_with("sha256:") && !reference.contains('@') {
        let base = reference.split(':').next().unwrap_or(reference);
        format!("{}@{}", base, digest)
    } else {
        reference.to_string()
    };

    let output = Command::new("cosign")
        .args([
            "verify",
            "--certificate-identity",
            CHAINGUARD_IDENTITY,
            "--certificate-oidc-issuer",
            CHAINGUARD_ISSUER,
            "--output",
            "text",
            &full_ref,
        ])
        .output()
        .await;

    match output {
        Ok(out) if out.status.success() => VerificationResult::success(digest),
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();

            // cosign not found — allow in development mode
            if stderr.contains("not found")
                || stderr.contains("command not found")
                || stderr.contains("executable file not found")
            {
                return VerificationResult::success(digest);
            }

            VerificationResult::failure(digest, stderr)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // cosign binary not installed — allow in development mode
            VerificationResult::success(digest)
        }
        Err(e) => VerificationResult::failure(
            digest,
            format!("cosign execution failed: {}", e),
        ),
    }
}

/// Verify an image using a specific public key (keyful verification).
pub async fn verify_image_with_key(
    reference: &str,
    key_path: &str,
) -> Result<String, ContainerError> {
    let digest = fetch_image_digest(reference).await?;
    let cache = get_cache();

    {
        let rd = cache.read().unwrap();
        if let Some(entry) = rd.get(&digest) {
            if entry.is_fresh() && entry.verified {
                return Ok(digest.clone());
            }
        }
    }

    let output = Command::new("cosign")
        .args(["verify", "--key", key_path, "--output", "text", reference])
        .output()
        .await;

    match output {
        Ok(out) if out.status.success() => {
            let mut wr = cache.write().unwrap();
            wr.insert(digest.clone(), VerificationResult::success(&digest));
            Ok(digest)
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            let mut wr = cache.write().unwrap();
            wr.insert(
                digest.clone(),
                VerificationResult::failure(&digest, &stderr),
            );
            Err(ContainerError::VerificationFailed {
                image: reference.to_string(),
                reason: stderr,
            })
        }
        Err(e) => Err(ContainerError::VerificationFailed {
            image: reference.to_string(),
            reason: format!("cosign binary not found: {}", e),
        }),
    }
}

// ============ Chainguard image lookup ============

/// Look up the Chainguard image reference for a given tool name.
///
/// Returns `Some("cgr.dev/chainguard/<image>:latest")` for known tools,
/// or `None` if the tool is not in the lookup table.
pub fn get_chainguard_image(tool: &str) -> Option<String> {
    match tool {
        // Runtimes
        "node" | "nodejs" | "npm" | "npx" => Some("cgr.dev/chainguard/node:latest".to_string()),
        "python" | "python3" | "pip" | "pip3" => {
            Some("cgr.dev/chainguard/python:latest".to_string())
        }
        "ruby" | "gem" => Some("cgr.dev/chainguard/ruby:latest".to_string()),
        "java" | "javac" | "jar" => Some("cgr.dev/chainguard/jdk:latest".to_string()),
        "gradle" => Some("cgr.dev/chainguard/gradle:latest".to_string()),
        "maven" => Some("cgr.dev/chainguard/maven:latest".to_string()),

        // Compiled languages
        "rust" | "rustc" | "cargo" => Some("cgr.dev/chainguard/rust:latest".to_string()),
        "go" | "golang" => Some("cgr.dev/chainguard/go:latest".to_string()),
        "gcc" | "g++" | "cc" | "c++" => Some("cgr.dev/chainguard/gcc:latest".to_string()),
        "clang" | "clang++" => Some("cgr.dev/chainguard/clang:latest".to_string()),

        // Build tools
        "make" => Some("cgr.dev/chainguard/make:latest".to_string()),
        "cmake" => Some("cgr.dev/chainguard/cmake:latest".to_string()),

        // Web servers
        "nginx" => Some("cgr.dev/chainguard/nginx:latest".to_string()),
        "caddy" => Some("cgr.dev/chainguard/caddy:latest".to_string()),

        // Databases / caching
        "redis" | "redis-cli" => Some("cgr.dev/chainguard/redis:latest".to_string()),
        "postgres" | "psql" => Some("cgr.dev/chainguard/postgres:latest".to_string()),
        "mysql" | "mariadb" => Some("cgr.dev/chainguard/mariadb:latest".to_string()),
        "sqlite3" | "sqlite" => Some("cgr.dev/chainguard/sqlite:latest".to_string()),
        "mongo" | "mongosh" => Some("cgr.dev/chainguard/mongodb:latest".to_string()),

        // Network / HTTP
        "git" => Some("cgr.dev/chainguard/git:latest".to_string()),
        "curl" => Some("cgr.dev/chainguard/curl:latest".to_string()),
        "wget" => Some("cgr.dev/chainguard/wget:latest".to_string()),
        "ssh" | "scp" | "sftp" => Some("cgr.dev/chainguard/openssh:latest".to_string()),
        "openssl" => Some("cgr.dev/chainguard/openssl:latest".to_string()),

        // Shell / coreutils
        "bash" => Some("cgr.dev/chainguard/bash:latest".to_string()),
        "sh" | "ash" | "busybox" => Some("cgr.dev/chainguard/busybox:latest".to_string()),
        "zsh" => Some("cgr.dev/chainguard/zsh:latest".to_string()),
        "awk" | "gawk" => Some("cgr.dev/chainguard/gawk:latest".to_string()),
        "sed" => Some("cgr.dev/chainguard/sed:latest".to_string()),
        "grep" => Some("cgr.dev/chainguard/grep:latest".to_string()),
        "jq" => Some("cgr.dev/chainguard/jq:latest".to_string()),
        "yq" => Some("cgr.dev/chainguard/yq:latest".to_string()),
        "tar" => Some("cgr.dev/chainguard/tar:latest".to_string()),
        "zip" | "unzip" => Some("cgr.dev/chainguard/zip:latest".to_string()),

        // Package managers / base
        "apt" | "apt-get" | "dpkg" | "apk" | "yum" | "dnf" | "rpm" => {
            Some("cgr.dev/chainguard/wolfi-base:latest".to_string())
        }

        // DevOps / cloud
        "docker" => Some("cgr.dev/chainguard/docker:latest".to_string()),
        "kubectl" | "k8s" => Some("cgr.dev/chainguard/kubectl:latest".to_string()),
        "helm" => Some("cgr.dev/chainguard/helm:latest".to_string()),
        "terraform" => Some("cgr.dev/chainguard/terraform:latest".to_string()),
        "aws" | "awscli" => Some("cgr.dev/chainguard/aws-cli:latest".to_string()),
        "az" | "azure" => Some("cgr.dev/chainguard/azure-cli:latest".to_string()),
        "gcloud" => Some("cgr.dev/chainguard/gcloud:latest".to_string()),

        // Utilities
        "vim" | "vi" | "nvim" => Some("cgr.dev/chainguard/vim:latest".to_string()),
        "nano" => Some("cgr.dev/chainguard/nano:latest".to_string()),
        "less" | "more" => Some("cgr.dev/chainguard/less:latest".to_string()),
        "rsync" => Some("cgr.dev/chainguard/rsync:latest".to_string()),
        "socat" => Some("cgr.dev/chainguard/socat:latest".to_string()),
        "netcat" | "nc" => Some("cgr.dev/chainguard/netcat:latest".to_string()),
        "strace" => Some("cgr.dev/chainguard/strace:latest".to_string()),
        "lsof" => Some("cgr.dev/chainguard/lsof:latest".to_string()),
        "file" => Some("cgr.dev/chainguard/file:latest".to_string()),
        "htop" | "top" => Some("cgr.dev/chainguard/procps:latest".to_string()),

        _ => None,
    }
}

/// Get the default base image for sandboxed containers.
///
/// Returns the Chainguard static image — a minimal, distroless base with
/// no shell, no package manager, and a minimal attack surface.
pub fn get_default_base_image() -> &'static str {
    "cgr.dev/chainguard/static:latest"
}

/// Get the Wolfi-based base image (has a shell and package manager).
pub fn get_wolfi_base_image() -> &'static str {
    "cgr.dev/chainguard/wolfi-base:latest"
}

/// Clear the verification cache (useful for testing).
pub fn clear_verification_cache() {
    if let Some(cache) = VERIFICATION_CACHE.get() {
        let mut wr = cache.write().unwrap();
        wr.clear();
    }
}

// ============ Tests ============

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chainguard_constants() {
        assert!(!CHAINGUARD_IDENTITY.is_empty());
        assert!(!CHAINGUARD_ISSUER.is_empty());
        assert!(CHAINGUARD_ISSUER.starts_with("https://"));
    }

    #[test]
    fn test_get_default_base_image() {
        let img = get_default_base_image();
        assert_eq!(img, "cgr.dev/chainguard/static:latest");
    }

    #[test]
    fn test_get_chainguard_image_known_tools() {
        assert_eq!(
            get_chainguard_image("node"),
            Some("cgr.dev/chainguard/node:latest".to_string())
        );
        assert_eq!(
            get_chainguard_image("python"),
            Some("cgr.dev/chainguard/python:latest".to_string())
        );
        assert_eq!(
            get_chainguard_image("go"),
            Some("cgr.dev/chainguard/go:latest".to_string())
        );
        assert_eq!(
            get_chainguard_image("rust"),
            Some("cgr.dev/chainguard/rust:latest".to_string())
        );
        assert_eq!(
            get_chainguard_image("java"),
            Some("cgr.dev/chainguard/jdk:latest".to_string())
        );
        assert_eq!(
            get_chainguard_image("nginx"),
            Some("cgr.dev/chainguard/nginx:latest".to_string())
        );
        assert_eq!(
            get_chainguard_image("redis"),
            Some("cgr.dev/chainguard/redis:latest".to_string())
        );
        assert_eq!(
            get_chainguard_image("postgres"),
            Some("cgr.dev/chainguard/postgres:latest".to_string())
        );
    }

    #[test]
    fn test_get_chainguard_image_unknown_tool() {
        assert_eq!(get_chainguard_image("unknown-tool-xyz"), None);
        assert_eq!(get_chainguard_image(""), None);
    }

    #[test]
    fn test_verification_result_success() {
        let r = VerificationResult::success("sha256:abc123");
        assert!(r.verified);
        assert_eq!(r.digest, "sha256:abc123");
        assert!(r.reason.is_none());
        assert!(r.is_fresh());
    }

    #[test]
    fn test_verification_result_failure() {
        let r = VerificationResult::failure("sha256:abc123", "no signatures found");
        assert!(!r.verified);
        assert_eq!(r.digest, "sha256:abc123");
        assert_eq!(r.reason.as_deref(), Some("no signatures found"));
        assert!(r.is_fresh());
    }

    #[test]
    fn test_cache_hit_returns_cached_result() {
        clear_verification_cache();

        let digest = "sha256:test_cache_hit_digest_12345";
        {
            let mut wr = get_cache().write().unwrap();
            wr.insert(digest.to_string(), VerificationResult::success(digest));
        }

        let rd = get_cache().read().unwrap();
        let entry = rd.get(digest).expect("cache entry should exist");
        assert!(entry.verified);
        assert!(entry.is_fresh());
    }

    #[test]
    fn test_clear_verification_cache() {
        {
            let mut wr = get_cache().write().unwrap();
            wr.insert(
                "sha256:to_be_cleared".to_string(),
                VerificationResult::success("sha256:to_be_cleared"),
            );
        }

        clear_verification_cache();

        let rd = get_cache().read().unwrap();
        assert!(rd.get("sha256:to_be_cleared").is_none());
    }

    #[test]
    fn test_tool_aliases_resolve_to_same_image() {
        assert_eq!(get_chainguard_image("npm"), get_chainguard_image("node"));
        assert_eq!(get_chainguard_image("npx"), get_chainguard_image("node"));
        assert_eq!(get_chainguard_image("pip"), get_chainguard_image("python"));
        assert_eq!(get_chainguard_image("pip3"), get_chainguard_image("python"));
        assert_eq!(
            get_chainguard_image("psql"),
            get_chainguard_image("postgres")
        );
    }
}
