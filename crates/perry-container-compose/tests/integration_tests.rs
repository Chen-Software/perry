//! Integration tests for perry-container-compose.
//!
//! These tests require a running container backend and are gated
//! by `#[cfg(feature = "integration-tests")]`.
//!
//! Real integration tests that shell out to the backend should be placed here.

#[cfg(feature = "integration-tests")]
mod integration {
    use perry_container_compose::backend::detect_backend;

    #[tokio::test]
    async fn test_backend_availability() {
        let backend = detect_backend().await.expect("No container backend found for integration tests");
        backend.check_available().await.expect("Detected backend is not functional");
    }
}
