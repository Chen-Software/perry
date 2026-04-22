use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
use perry_container_compose::error::ComposeError;
use tokio::process::Command;

pub const CHAINGUARD_IDENTITY: &str = "https://github.com/chainguard-images/images/.github/workflows/sign.yaml@refs/heads/main";
pub const CHAINGUARD_ISSUER: &str = "https://token.actions.githubusercontent.com";

#[derive(Debug, Clone)]
enum VerificationResult { Verified(String), Failed(String) }
static VERIFICATION_CACHE: OnceLock<RwLock<HashMap<String, VerificationResult>>> = OnceLock::new();

pub async fn verify_image(reference: &str) -> Result<String, String> {
    let digest = if reference.contains('@') { reference.split('@').last().unwrap().to_string() }
    else {
        let out = Command::new("crane").args(&["digest", reference]).output().await.map_err(|e| e.to_string())?;
        if !out.status.success() { return Err(format!("crane digest failed: {}", String::from_utf8_lossy(&out.stderr))); }
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    };
    let cache = VERIFICATION_CACHE.get_or_init(|| RwLock::new(HashMap::new()));
    if let Some(res) = cache.read().unwrap().get(&digest) { match res { VerificationResult::Verified(d) => return Ok(d.clone()), VerificationResult::Failed(r) => return Err(r.clone()) } }
    let out = Command::new("cosign").args(&["verify", "--certificate-identity", CHAINGUARD_IDENTITY, "--certificate-oidc-issuer", CHAINGUARD_ISSUER, reference]).output().await.map_err(|e| e.to_string())?;
    let res = if out.status.success() { VerificationResult::Verified(digest.clone()) } else { VerificationResult::Failed(format!("cosign failed: {}", String::from_utf8_lossy(&out.stderr))) };
    cache.write().unwrap().insert(digest.clone(), res.clone());
    match res { VerificationResult::Verified(d) => Ok(d), VerificationResult::Failed(r) => Err(r) }
}

pub fn get_default_base_image() -> &'static str { "cgr.dev/chainguard/alpine-base" }
pub fn get_chainguard_image(tool: &str) -> Option<String> { match tool { "node" => Some("cgr.dev/chainguard/node".into()), "python" => Some("cgr.dev/chainguard/python".into()), _ => None } }
