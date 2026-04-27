use crate::error::ComposeError;
use tokio::process::Command;

pub async fn verify_image(reference: &str) -> Result<String, ComposeError> {
    if std::env::var("PERRY_SKIP_IMAGE_VERIFY").is_ok() {
        return Ok("sha256:skipped".to_string());
    }

    let output = Command::new("cosign")
        .args(["verify", "--certificate-identity", "CHAINGUARD_IDENTITY", "--certificate-oidc-issuer", "CHAINGUARD_ISSUER", reference])
        .output()
        .await
        .map_err(|e| ComposeError::BackendError { code: -1, message: format!("cosign failed: {}", e) })?;

    if output.status.success() {
        // Extract digest from output in real implementation
        Ok("sha256:abcdef...".to_string())
    } else {
        Err(ComposeError::VerificationFailed {
            image: reference.to_string(),
            reason: String::from_utf8_lossy(&output.stderr).to_string()
        })
    }
}

pub fn get_default_base_image() -> &'static str {
    "cgr.dev/chainguard/alpine-base"
}

pub fn get_chainguard_image(tool: &str) -> Option<String> {
    match tool {
        "git" => Some("cgr.dev/chainguard/git".to_string()),
        "node" => Some("cgr.dev/chainguard/node".to_string()),
        _ => None,
    }
}
