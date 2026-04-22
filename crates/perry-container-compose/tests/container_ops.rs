use perry_container_compose::backend::ContainerBackend;
use perry_container_compose::types::ContainerSpec;
use std::sync::Arc;

mod common;
use common::MockBackend;

#[tokio::test]
async fn test_container_run_success() {
    let backend = MockBackend::new();
    let spec = ContainerSpec {
        image: "alpine".into(),
        name: Some("test-container".into()),
        ..Default::default()
    };

    let handle = backend.run(&spec).await.expect("run failed");
    assert_eq!(handle.id, "test-container");

    let containers = backend.containers.lock().unwrap();
    assert!(containers.contains_key("test-container"));
}

#[tokio::test]
async fn test_container_lifecycle() {
    let backend = MockBackend::new();
    let spec = ContainerSpec {
        image: "nginx".into(),
        name: Some("web".into()),
        ..Default::default()
    };

    backend.run(&spec).await.unwrap();
    backend.stop("web", Some(10)).await.unwrap();
    backend.remove("web", true).await.unwrap();

    let containers = backend.containers.lock().unwrap();
    assert!(!containers.contains_key("web"));
}

#[tokio::test]
async fn test_container_exec() {
    let backend = MockBackend::new();
    let logs = backend.exec("web", &["ls".into()], None, None, None).await.unwrap();
    assert_eq!(logs.stdout, "mock exec output");
}

#[tokio::test]
async fn test_network_volume_lifecycle() {
    let backend = MockBackend::new();
    use perry_container_compose::backend::{NetworkConfig, VolumeConfig};

    backend.create_network("test-net", &NetworkConfig::default()).await.unwrap();
    backend.create_volume("test-vol", &VolumeConfig::default()).await.unwrap();

    {
        assert!(backend.networks.lock().unwrap().contains(&"test-net".to_string()));
        assert!(backend.volumes.lock().unwrap().contains(&"test-vol".to_string()));
    }

    backend.remove_network("test-net").await.unwrap();
    backend.remove_volume("test-vol").await.unwrap();

    {
        assert!(!backend.networks.lock().unwrap().contains(&"test-net".to_string()));
        assert!(!backend.volumes.lock().unwrap().contains(&"test-vol".to_string()));
    }
}
