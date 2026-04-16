// Feature: perry-container | Layer: integration | Req: 1.1 | Property: -

#[cfg(feature = "integration-tests")]
mod integration {
    use perry_container_compose::backend::*;
    use perry_container_compose::compose::ComposeEngine;
    use perry_container_compose::types::*;
    use std::sync::Arc;

    async fn get_backend() -> Option<Arc<dyn ContainerBackend>> {
        let b = detect_backend().await.ok()?;
        Some(Arc::new(b))
    }

    // Feature: perry-container | Layer: integration | Req: 1.1 | Property: -
    #[tokio::test]
    #[ignore]
    async fn test_backend_detection_smoke() {
        let backend = detect_backend().await;
        if backend.is_err() { return; }
        let backend = backend.unwrap();
        assert!(!backend.backend_name().is_empty());
    }

    // Feature: perry-container | Layer: integration | Req: 2.1 | Property: -
    #[tokio::test]
    #[ignore]
    async fn test_single_container_lifecycle() {
        let backend = match get_backend().await { Some(b) => b, None => return };

        let spec = ContainerSpec {
            image: "alpine:latest".into(),
            name: Some(format!("perry-test-{}", rand::random::<u32>())),
            cmd: Some(vec!["sleep".into(), "10".into()]),
            rm: Some(true),
            ..Default::default()
        };

        // Pull, run, inspect, stop
        backend.pull_image(&spec.image).await.expect("Pull failed");
        let handle = backend.run(&spec).await.expect("Run failed");
        let info = backend.inspect(&handle.id).await.expect("Inspect failed");
        assert!(info.status.contains("Up") || info.status.contains("running"));

        backend.stop(&handle.id, Some(1)).await.expect("Stop failed");
        // rm: true should handle cleanup
    }

    // Feature: perry-container | Layer: integration | Req: 6.1 | Property: -
    #[tokio::test]
    #[ignore]
    async fn test_compose_up_down_cycle() {
        let backend = match get_backend().await { Some(b) => b, None => return };
        let project_name = format!("perry-proj-{}", rand::random::<u32>());

        let yaml = r#"
services:
  web:
    image: nginx:alpine
    ports:
      - "8081:80"
  db:
    image: redis:alpine
"#;
        let spec = ComposeSpec::parse_str(yaml).unwrap();
        let engine = ComposeEngine::new(spec, project_name.clone(), backend.clone());

        // Up
        let handle = engine.up(&[], true, false, false).await.expect("Up failed");
        assert_eq!(handle.services.len(), 2);

        // Ps
        let ps = engine.ps().await.expect("Ps failed");
        assert!(ps.len() >= 2);

        // Exec
        let exec_res = engine.exec("web", &["nginx".into(), "-v".into()], None, None).await;
        assert!(exec_res.is_ok());

        // Down
        engine.down(&[], false, true).await.expect("Down failed");

        let ps_after = engine.ps().await.unwrap();
        assert_eq!(ps_after.len(), 0);
    }

    // Feature: perry-container | Layer: integration | Req: 6.10 | Property: -
    #[tokio::test]
    #[ignore]
    async fn test_compose_rollback_on_failure() {
        let backend = match get_backend().await { Some(b) => b, None => return };
        let project_name = format!("perry-fail-{}", rand::random::<u32>());

        // One good service, one bad image
        let yaml = r#"
services:
  good:
    image: alpine:latest
    command: sleep 60
  bad:
    image: nonexistent-image-perry-12345
    depends_on: [good]
"#;
        let spec = ComposeSpec::parse_str(yaml).unwrap();
        let engine = ComposeEngine::new(spec, project_name.clone(), backend.clone());

        // Up should fail
        let result = engine.up(&[], true, false, false).await;
        assert!(result.is_err());

        // Rollback should have removed 'good' container
        let ps = backend.list(true).await.unwrap();
        let found_good = ps.iter().any(|c| c.name.contains("good") && c.name.contains(&project_name));
        assert!(!found_good, "Good service should have been rolled back");
    }
}

/*
| Requirement | Test name | Layer |
|-------------|-----------|-------|
| 1.1         | test_backend_detection_smoke | integration |
| 2.1         | test_single_container_lifecycle | integration |
| 3.2         | test_single_container_lifecycle | integration |
| 5.1         | test_single_container_lifecycle | integration |
| 6.1         | test_compose_up_down_cycle | integration |
| 6.6         | test_compose_up_down_cycle | integration |
| 6.7         | test_compose_up_down_cycle | integration |
| 6.10        | test_compose_rollback_on_failure | integration |
*/

// Deferred Requirements:
// Req 13.1 — alloy_container_run_capability() requires ShellBridge context.
// Req 15.1 — Image verification requires cosign binary and network access.
