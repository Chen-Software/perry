use perry_container_compose::error::{ComposeError, compose_error_to_js};

// Feature: perry-container | Layer: unit | Req: 2.6 | Property: 11
#[test]
fn test_compose_error_not_found_mapping() {
    let err = ComposeError::NotFound("foo".into());
    let js = compose_error_to_js(&err);
    assert!(js.contains("\"code\":404"));
    assert!(js.contains("Not found: foo"));
}

// Feature: perry-container | Layer: unit | Req: 6.5 | Property: 11
#[test]
fn test_compose_error_dependency_cycle_mapping() {
    let err = ComposeError::DependencyCycle { services: vec!["a".into(), "b".into()] };
    let js = compose_error_to_js(&err);
    assert!(js.contains("\"code\":422"));
    assert!(js.contains("Dependency cycle detected"));
}

// Feature: perry-container | Layer: unit | Req: 12.2 | Property: 11
#[test]
fn test_compose_error_backend_error_mapping() {
    let err = ComposeError::BackendError { code: 125, message: "backend failed".into() };
    let js = compose_error_to_js(&err);
    assert!(js.contains("\"code\":125"));
    assert!(js.contains("backend failed"));
}

// Feature: perry-container | Layer: unit | Req: 6.10 | Property: 11
#[test]
fn test_compose_error_startup_failed_mapping() {
    let err = ComposeError::ServiceStartupFailed { service: "web".into(), message: "port taken".into() };
    let js = compose_error_to_js(&err);
    assert!(js.contains("\"code\":500"));
    assert!(js.contains("Service 'web' failed to start"));
}

// Feature: perry-container | Layer: unit | Req: 16.11 | Property: 11
#[test]
fn test_compose_error_no_backend_found_mapping() {
    let err = ComposeError::NoBackendFound { probed: vec![] };
    let js = compose_error_to_js(&err);
    assert!(js.contains("\"code\":503"));
}

// Feature: perry-container | Layer: unit | Req: none | Property: 11
#[test]
fn test_compose_error_validation_error_mapping() {
    let err = ComposeError::ValidationError { message: "invalid spec".into() };
    let js = compose_error_to_js(&err);
    assert!(js.contains("\"code\":400"));
}

// Feature: perry-container | Layer: unit | Req: none | Property: 11
#[test]
fn test_compose_error_verification_failed_mapping() {
    let err = ComposeError::VerificationFailed { image: "img".into(), reason: "bad sig".into() };
    let js = compose_error_to_js(&err);
    assert!(js.contains("\"code\":403"));
}

// Feature: perry-container | Layer: unit | Req: none | Property: 11
#[test]
fn test_compose_error_file_not_found_mapping() {
    let err = ComposeError::FileNotFound { path: "compose.yml".into() };
    let js = compose_error_to_js(&err);
    // FileNotFound matches 404 per SPEC 2.6
    assert!(js.contains("\"code\":404"));
}
