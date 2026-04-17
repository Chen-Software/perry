use perry_container_compose::error::{ComposeError, compose_error_to_js};
use perry_container_compose::backend::BackendProbeResult;

// Feature: perry-container | Layer: unit | Req: 2.6 | Property: 11
#[test]
fn test_error_not_found_mapping() {
    let err = ComposeError::NotFound("id".into());
    let js = compose_error_to_js(&err);
    assert!(js.contains("\"code\":404"));
}

// Feature: perry-container | Layer: unit | Req: 6.5 | Property: 11
#[test]
fn test_error_dependency_cycle_mapping() {
    let err = ComposeError::DependencyCycle { services: vec!["a".into()] };
    let js = compose_error_to_js(&err);
    assert!(js.contains("\"code\":422"));
}

// Feature: perry-container | Layer: unit | Req: 12.2 | Property: 11
#[test]
fn test_error_backend_error_mapping() {
    let err = ComposeError::BackendError { code: 125, message: "err".into() };
    let js = compose_error_to_js(&err);
    assert!(js.contains("\"code\":125"));
}

// Feature: perry-container | Layer: unit | Req: 16.11 | Property: 11
#[test]
fn test_error_no_backend_mapping() {
    let err = ComposeError::NoBackendFound { probed: vec![] };
    let js = compose_error_to_js(&err);
    assert!(js.contains("\"code\":503"));
}

// Feature: perry-container | Layer: unit | Req: 6.10 | Property: 11
#[test]
fn test_error_startup_failed_mapping() {
    let err = ComposeError::ServiceStartupFailed { service: "s".into(), message: "m".into() };
    let js = compose_error_to_js(&err);
    assert!(js.contains("\"code\":500"));
}

// Feature: perry-container | Layer: unit | Req: none | Property: 11
#[test]
fn test_error_validation_mapping() {
    let err = ComposeError::ValidationError { message: "v".into() };
    let js = compose_error_to_js(&err);
    assert!(js.contains("\"code\":400"));
}

// Feature: perry-container | Layer: unit | Req: none | Property: 11
#[test]
fn test_error_verification_failed_mapping() {
    let err = ComposeError::VerificationFailed { image: "i".into(), reason: "r".into() };
    let js = compose_error_to_js(&err);
    assert!(js.contains("\"code\":403"));
}

// Feature: perry-container | Layer: unit | Req: none | Property: 11
#[test]
fn test_error_not_available_mapping() {
    let err = ComposeError::BackendNotAvailable { name: "n".into(), reason: "r".into() };
    let js = compose_error_to_js(&err);
    assert!(js.contains("\"code\":503"));
}

// Feature: perry-container | Layer: unit | Req: none | Property: 11
#[test]
fn test_error_io_mapping() {
    let io_err = std::io::Error::new(std::io::ErrorKind::Other, "e");
    let err = ComposeError::IoError(io_err);
    let js = compose_error_to_js(&err);
    assert!(js.contains("\"code\":500"));
}
