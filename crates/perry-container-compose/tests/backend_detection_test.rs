use perry_container_compose::backend::{detect_backend, BackendProbeResult};
use perry_container_compose::error::ComposeError;

#[tokio::test]
async fn test_detect_backend_env_override() {
    std::env::set_var("PERRY_CONTAINER_BACKEND", "nonexistent");
    let res = detect_backend().await;
    match res {
        Err(ComposeError::BackendNotAvailable { name, .. }) => {
            assert_eq!(name, "nonexistent");
        }
        _ => panic!("Expected BackendNotAvailable error"),
    }
    std::env::remove_var("PERRY_CONTAINER_BACKEND");
}
