use perry_container_compose::backend::detect_backend;
use perry_container_compose::compose::ComposeEngine;
use perry_container_compose::types::ComposeSpec;
use perry_container_compose::ContainerBackend;
use std::sync::Arc;

// Feature: perry-container | Layer: integration | Req: 6.1 | Property: -
#[cfg(feature = "integration-tests")]
#[tokio::test]
#[ignore]
async fn test_compose_up_down_integration() {
    let backend_res = detect_backend().await;
    if backend_res.is_err() { return; }
    let backend = Arc::new(backend_res.unwrap());

    let yaml = r#"
services:
  web:
    image: alpine
    command: ["sleep", "60"]
"#;
    let spec = ComposeSpec::parse_str(yaml).unwrap();
    let project_name = format!("test-project-{}", rand::random::<u32>());
    let engine = ComposeEngine::new(spec, project_name.clone(), backend.clone());

    // up
    let handle = engine.up(&[], true, false, false).await.expect("Up should succeed");
    assert_eq!(handle.project_name, project_name);

    // ps
    let containers = engine.ps().await.expect("Ps should succeed");
    assert!(containers.iter().any(|c| c.image.contains("alpine")));

    // down
    engine.down(&[], false, true).await.expect("Down should succeed");
}

// Feature: perry-container | Layer: integration | Req: 6.6 | Property: -
#[cfg(feature = "integration-tests")]
#[tokio::test]
#[ignore]
async fn test_container_exec_integration() {
    let backend_res = detect_backend().await;
    if backend_res.is_err() { return; }
    let backend = Arc::new(backend_res.unwrap());

    let spec = perry_container_compose::types::ContainerSpec {
        image: "alpine".into(),
        cmd: Some(vec!["sleep".into(), "60".into()]),
        ..Default::default()
    };

    let handle = backend.run(&spec).await.expect("Run should succeed");

    let logs = backend.exec(&handle.id, &vec!["echo".into(), "hello".into()], None, None).await.expect("Exec should succeed");
    assert!(logs.stdout.contains("hello"));

    backend.stop(&handle.id, Some(0)).await.ok();
    backend.remove(&handle.id, true).await.ok();
}
