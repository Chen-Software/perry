use perry_container_compose::compose::ComposeEngine;
use perry_container_compose::types::{ComposeSpec, ComposeService};
use std::sync::Arc;

mod common;
use common::MockBackend;

#[tokio::test]
async fn test_compose_up_success() {
    let mut spec = ComposeSpec::default();
    spec.services.insert("web".into(), ComposeService {
        image: Some("nginx".into()),
        ..Default::default()
    });
    spec.services.insert("db".into(), ComposeService {
        image: Some("postgres".into()),
        ..Default::default()
    });

    let backend = Arc::new(MockBackend::new());
    let engine = ComposeEngine::new(spec, "test-project".into(), backend.clone());

    let handle = engine.up(&[], true, false, false).await.expect("up failed");

    assert_eq!(handle.project_name, "test-project");
    assert_eq!(handle.services.len(), 2);

    let containers = backend.containers.lock().unwrap();
    assert_eq!(containers.len(), 2);
}

#[tokio::test]
async fn test_compose_down_cleans_resources() {
    let mut spec = ComposeSpec::default();
    spec.services.insert("web".into(), ComposeService {
        image: Some("nginx".into()),
        ..Default::default()
    });

    let backend = Arc::new(MockBackend::new());
    let engine = ComposeEngine::new(spec, "down-project".into(), backend.clone());

    let handle = engine.up(&[], true, false, false).await.unwrap();
    engine.down(&handle.services, false, true).await.expect("down failed");

    let containers = backend.containers.lock().unwrap();
    assert!(containers.is_empty(), "Containers should be empty, but found: {:?}", containers);
}
