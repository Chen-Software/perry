use perry_container_compose::backend::detect_backend;
use perry_container_compose::types::ContainerSpec;
use std::env;

#[tokio::test]
#[cfg(feature = "integration-tests")]
async fn test_container_lifecycle_alpine() {
    if env::var("PERRY_INTEGRATION_TESTS").is_err() {
        return;
    }

    let driver = detect_backend().await.expect("No backend found");
    let backend = driver.instantiate();

    let spec = ContainerSpec {
        image: "alpine:latest".to_string(),
        cmd: Some(vec!["echo".to_string(), "hello".to_string()]),
        rm: Some(true),
        ..Default::default()
    };

    let handle = backend.run(&spec).await.expect("Failed to run alpine");
    assert!(!handle.id.is_empty());

    let logs = backend.logs(&handle.id, None).await.expect("Failed to get logs");
    assert!(logs.stdout.contains("hello"));
}
