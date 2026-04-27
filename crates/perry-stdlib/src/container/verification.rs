use crate::container::types::ContainerError;

pub async fn verify_image(reference: &str) -> Result<String, ContainerError> {
    // Mock verification for now
    if std::env::var("PERRY_SKIP_IMAGE_VERIFY").is_ok() {
        return Ok("sha256:digest".into());
    }
    Ok("sha256:digest".into())
}

pub fn get_default_base_image() -> &'static str {
    "cgr.dev/chainguard/alpine-base"
}
