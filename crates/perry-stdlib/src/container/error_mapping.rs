use perry_container_compose::error::ComposeError;

pub fn compose_error_to_js(e: ComposeError) -> String {
    let code = match &e {
        ComposeError::NotFound(_) => 404,
        ComposeError::BackendError { code, .. } => *code,
        ComposeError::DependencyCycle { .. } => 422,
        ComposeError::ValidationError { .. } => 400,
        ComposeError::VerificationFailed { .. } => 403,
        ComposeError::NoBackendFound { .. } => 503,
        ComposeError::BackendNotAvailable { .. } => 503,
        _ => 500,
    };
    serde_json::json!({
        "message": e.to_string(),
        "code": code
    }).to_string()
}
