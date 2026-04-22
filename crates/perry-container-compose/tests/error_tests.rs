//! Tests for the `error` module.
//!
//! Validates `ComposeError` serialization to JS/JSON and code mapping.

use perry_container_compose::error::{compose_error_to_js, ComposeError, BackendProbeResult};
use serde_json::Value;

// Feature: perry-container | Layer: unit | Req: 12.2 | Property: 11
#[test]
fn test_compose_error_to_js_not_found() {
    let err = ComposeError::NotFound("container-123".to_string());
    let json_str = compose_error_to_js(&err);
    let v: Value = serde_json::from_str(&json_str).unwrap();

    assert_eq!(v["code"], 404);
    assert!(v["message"].as_str().unwrap().contains("Not found: container-123"));
}

// Feature: perry-container | Layer: unit | Req: 6.5 | Property: 11
#[test]
fn test_compose_error_to_js_dependency_cycle() {
    let err = ComposeError::DependencyCycle { services: vec!["a".to_string(), "b".to_string()] };
    let json_str = compose_error_to_js(&err);
    let v: Value = serde_json::from_str(&json_str).unwrap();

    assert_eq!(v["code"], 422);
    assert!(v["message"].as_str().unwrap().contains("Dependency cycle detected"));
}

// Feature: perry-container | Layer: unit | Req: 2.6 | Property: 11
#[test]
fn test_compose_error_to_js_backend_error() {
    let err = ComposeError::BackendError { code: 125, message: "daemon not running".to_string() };
    let json_str = compose_error_to_js(&err);
    let v: Value = serde_json::from_str(&json_str).unwrap();

    assert_eq!(v["code"], 125);
    assert!(v["message"].as_str().unwrap().contains("daemon not running"));
}

// Feature: perry-container | Layer: unit | Req: none | Property: 11
#[test]
fn test_compose_error_to_js_validation_error() {
    let err = ComposeError::validation("invalid name");
    let json_str = compose_error_to_js(&err);
    let v: Value = serde_json::from_str(&json_str).unwrap();

    assert_eq!(v["code"], 400);
    assert!(v["message"].as_str().unwrap().contains("Validation error: invalid name"));
}

// Feature: perry-container | Layer: unit | Req: none | Property: 11
#[test]
fn test_compose_error_to_js_verification_failed() {
    let err = ComposeError::VerificationFailed { image: "img".to_string(), reason: "bad signature".to_string() };
    let json_str = compose_error_to_js(&err);
    let v: Value = serde_json::from_str(&json_str).unwrap();

    assert_eq!(v["code"], 403);
    assert!(v["message"].as_str().unwrap().contains("Image verification failed"));
}

// Feature: perry-container | Layer: unit | Req: 16.11 | Property: 11
#[test]
fn test_compose_error_to_js_no_backend_found() {
    let err = ComposeError::NoBackendFound { probed: vec![BackendProbeResult { name: "docker".to_string(), available: false, reason: "not found".to_string() }] };
    let json_str = compose_error_to_js(&err);
    let v: Value = serde_json::from_str(&json_str).unwrap();

    assert_eq!(v["code"], 503);
}

// Feature: perry-container | Layer: unit | Req: none | Property: 11
#[test]
fn test_compose_error_to_js_backend_not_available() {
    let err = ComposeError::BackendNotAvailable { name: "podman".to_string(), reason: "machine stopped".to_string() };
    let json_str = compose_error_to_js(&err);
    let v: Value = serde_json::from_str(&json_str).unwrap();

    assert_eq!(v["code"], 503);
}

// ============ Coverage Table ============
//
// | Requirement | Test name | Layer |
// |-------------|-----------|-------|
// | 2.6         | test_compose_error_to_js_backend_error | unit |
// | 6.5         | test_compose_error_to_js_dependency_cycle | unit |
// | 12.2        | test_compose_error_to_js_not_found | unit |
// | 16.11       | test_compose_error_to_js_no_backend_found | unit |
// | none        | test_compose_error_to_js_validation_error | unit |
// | none        | test_compose_error_to_js_verification_failed | unit |
// | none        | test_compose_error_to_js_backend_not_available | unit |
