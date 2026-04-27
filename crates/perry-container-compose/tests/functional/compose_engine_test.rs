use perry_container_compose::compose::ComposeEngine;
use perry_container_compose::types::*;
use perry_container_compose::testing::{MockBackend, RecordedCall};
use perry_container_compose::service::Service;
use indexmap::IndexMap;
use std::sync::Arc;

#[tokio::test]
async fn test_compose_up_basic() {
    let mock = Arc::new(MockBackend::new());

    let mut spec = ComposeSpec {
        name: Some("test".into()),
        version: None,
        services: IndexMap::new(),
        networks: None,
        volumes: None,
        secrets: None,
        configs: None,
    };

    spec.services.insert("web".into(), ComposeService {
        image: Some("nginx".into()),
        build: None,
        command: None,
        entrypoint: None,
        environment: None,
        env_file: None,
        ports: None,
        volumes: None,
        networks: None,
        depends_on: None,
        healthcheck: None,
        deploy: None,
        logging: None,
        container_name: Some("web-container".into()),
        labels: None,
        extra_hosts: None,
        sysctls: None,
        read_only: None,
        isolation_level: None,
    });

    let engine = ComposeEngine {
        backend: mock.clone(),
        services: spec.services.into_iter().map(|(k, v)| (k, Service::from(v))).collect(),
    };

    engine.up().await.unwrap();

    let calls = mock.calls.lock().unwrap();
    // print calls for debugging
    for (i, call) in calls.iter().enumerate() {
        println!("{}: {:?}", i, call);
    }
    assert!(calls.len() >= 2);
    let has_run = calls.iter().any(|c| matches!(c, RecordedCall::Run(_)));
    assert!(has_run);
}
