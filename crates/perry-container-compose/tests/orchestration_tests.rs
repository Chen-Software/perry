//! Comprehensive integration tests for perry/container and perry/compose.
//! These tests require a running container backend (Docker/Podman).

#[cfg(feature = "integration-tests")]
mod container_integration {
    use perry_container_compose::backend::{detect_backend, ContainerBackend};
    use perry_container_compose::compose::ComposeEngine;
    use perry_container_compose::types::{ComposeSpec, ContainerSpec};
    use std::sync::Arc;
    use tokio;

    async fn get_backend() -> Arc<dyn ContainerBackend> {
        let b = detect_backend().await.expect("No container backend found for integration tests");
        Arc::from(b)
    }

    #[tokio::test]
    async fn test_container_lifecycle() {
        let backend = get_backend().await;

        // 1. Run container
        let spec = ContainerSpec {
            image: "alpine:latest".to_string(),
            name: Some("perry-test-alpine".to_string()),
            cmd: Some(vec!["sleep".to_string(), "60".to_string()]),
            rm: Some(true),
            ..Default::default()
        };

        let handle = backend.run(&spec).await.expect("Failed to run container");
        assert!(!handle.id.is_empty());

        // 2. Inspect
        let info = backend.inspect(&handle.id).await.expect("Failed to inspect");
        assert_eq!(info.id, handle.id);
        assert!(info.status.to_lowercase().contains("running") || info.status.to_lowercase().contains("up"));

        // 3. Exec
        let logs = backend.exec(&handle.id, &["echo".to_string(), "hello".to_string()], None, None)
            .await.expect("Failed to exec");
        assert!(logs.stdout.contains("hello"));

        // 4. Stop
        backend.stop(&handle.id, Some(1)).await.expect("Failed to stop");

        // 5. List (verify it's gone if rm: true, or stopped)
        let list = backend.list(true).await.expect("Failed to list");
        let found = list.iter().any(|c| c.id == handle.id);
        // Note: with rm: true it might be gone immediately or shortly after stop
        if found {
            let info = backend.inspect(&handle.id).await.unwrap();
            assert!(info.status.to_lowercase().contains("exited") || info.status.to_lowercase().contains("stopped"));
            backend.remove(&handle.id, true).await.ok();
        }
    }

    #[tokio::test]
    async fn test_compose_orchestration() {
        let backend = get_backend().await;

        let yaml = r#"
name: perry-integration-test
services:
  web:
    image: nginx:alpine
    ports:
      - "8081:80"
    depends_on:
      - redis
  redis:
    image: redis:alpine
"#;
        let spec = ComposeSpec::parse_str(yaml).unwrap();
        let engine = ComposeEngine::new(spec, "perry-integration-test".into(), backend);

        // 1. Up
        let handle = engine.up(&[], true, false, false).await.expect("Compose up failed");
        assert_eq!(handle.services.len(), 2);

        // 2. PS
        let containers = engine.ps().await.expect("Compose ps failed");
        assert_eq!(containers.len(), 2);
        for c in &containers {
            assert!(c.status.to_lowercase().contains("running") || c.status.to_lowercase().contains("up"));
        }

        // 3. Logs
        let logs = engine.logs(&["web".to_string()], None).await.expect("Compose logs failed");
        assert!(logs.contains_key("web"));

        // 4. Down
        engine.down(&[], false, true).await.expect("Compose down failed");

        // 5. Verify gone
        let containers_after = engine.ps().await.unwrap();
        assert_eq!(containers_after.len(), 0);
    }

    #[tokio::test]
    async fn test_compose_rollback_on_failure() {
        let backend = get_backend().await;

        // One good service, one bad (invalid image)
        let yaml = r#"
name: perry-rollback-test
services:
  good:
    image: alpine:latest
    command: sleep 60
  bad:
    image: non-existent-image-perry-test-12345
    depends_on:
      - good
"#;
        let spec = ComposeSpec::parse_str(yaml).unwrap();
        let engine = ComposeEngine::new(spec, "perry-rollback-test".into(), backend.clone());

        // 1. Up should fail
        let result = engine.up(&[], true, false, false).await;
        assert!(result.is_err());

        // 2. Verify 'good' container was cleaned up by rollback
        let list = backend.list(true).await.unwrap();
        let found_good = list.iter().any(|c| c.name.contains("good"));
        assert!(!found_good, "Good service should have been rolled back");
    }
}
