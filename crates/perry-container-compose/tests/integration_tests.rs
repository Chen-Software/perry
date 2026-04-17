use perry_container_compose::backend::detect_backend;
use perry_container_compose::compose::ComposeEngine;
use perry_container_compose::types::ComposeSpec;
use std::sync::Arc;

// Feature: perry-container | Layer: integration | Req: 6.1 | Property: -
#[cfg(feature = "integration-tests")]
#[tokio::test]
#[ignore]
async fn test_compose_full_lifecycle_integration() {
    let backend = match detect_backend().await {
        Ok(b) => Arc::new(b),
        Err(_) => return,
    };

    let yaml = r#"
services:
  web:
    image: alpine:latest
    command: ["sleep", "60"]
"#;
    let spec: ComposeSpec = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
    let engine = ComposeEngine::new(spec, "int-test-stack".into(), backend);

    // 1. Up
    let handle = engine.up(&[], true, false, false).await.expect("Up failed");
    assert!(!handle.services.is_empty(), "Stack handle should have services");

    // 2. Ps
    let statuses = engine.ps().await.expect("Ps failed");
    assert!(statuses.iter().any(|s| s.image.contains("alpine")), "Should find alpine container");

    // 3. Down
    engine.down(&[], false, true).await.expect("Down failed");
}

// Feature: perry-container | Layer: integration | Req: 5.1 | Property: -
#[cfg(feature = "integration-tests")]
#[tokio::test]
#[ignore]
async fn test_container_exec_integration() {
    use perry_container_compose::types::ContainerSpec;
    let backend = match detect_backend().await {
        Ok(b) => Arc::new(b),
        Err(_) => return,
    };

    let spec = ContainerSpec {
        image: "alpine:latest".into(),
        cmd: Some(vec!["sleep".into(), "10".into()]),
        rm: Some(true),
        ..Default::default()
    };
    let handle = backend.run(&spec).await.expect("Run failed");

    let result = backend.exec(&handle.id, &["echo".into(), "hi-perry".into()], None, None).await.expect("Exec failed");
    assert!(result.stdout.contains("hi-perry"), "Exec stdout should contain hi-perry");

    let _ = backend.stop(&handle.id, Some(1)).await;
}
