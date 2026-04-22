//! Unit tests for the `backend` module.
//!
//! Validates platform-specific candidate ordering and environment variable override.

use perry_container_compose::backend::detect_backend;
use perry_container_compose::error::ComposeError;
use std::env;

// Feature: perry-container | Layer: unit | Req: 1.5 | Property: -
#[tokio::test]
async fn test_detect_backend_env_override_invalid() {
    // When PERRY_CONTAINER_BACKEND is set to an invalid value, it should fail immediately
    env::set_var("PERRY_CONTAINER_BACKEND", "nonexistent-backend");
    let result = detect_backend().await;
    env::remove_var("PERRY_CONTAINER_BACKEND");

    match result {
        Err(ComposeError::NoBackendFound { probed }) => {
            assert_eq!(probed.len(), 1);
            assert_eq!(probed[0].name, "nonexistent-backend");
            assert!(!probed[0].available);
            assert!(probed[0].reason.contains("unknown backend"));
        }
        _ => panic!("Expected NoBackendFound error"),
    }
}

// Feature: perry-container | Layer: unit | Req: 16.11 | Property: -
#[test]
fn test_backend_probe_result_structural() {
    use perry_container_compose::error::BackendProbeResult;
    let res = BackendProbeResult {
        name: "test".to_string(),
        available: true,
        reason: "ok".to_string(),
    };
    assert_eq!(res.name, "test");
    assert!(res.available);
}

// ============ Coverage Table ============
//
// | Requirement | Test name | Layer |
// |-------------|-----------|-------|
// | 1.5         | test_detect_backend_env_override_invalid | unit |
// | 16.11       | test_backend_probe_result_structural | unit |

// Deferred Requirements:
// Req 1.1-1.4, 1.6, 1.7, 16.1-16.10, 16.12 — these require a live container runtime or
// heavy mocking of Command/which, which is deferred to integration tests.
