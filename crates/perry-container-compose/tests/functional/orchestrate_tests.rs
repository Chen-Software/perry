use perry_container_compose::testing::mock_backend::{MockBackend, MockResponse};
use perry_container_compose::orchestrate::orchestrate_service;
use perry_container_compose::service::Service;
use perry_container_compose::types::ContainerInfo;
use std::sync::Arc;

#[tokio::test]
async fn test_orchestrate_skip_running() {
    let backend = MockBackend::new();
    let service = Service::new("web".into(), Some("nginx".into()));

    // Script inspect to return running
    backend.push_response(MockResponse::OkInfo(ContainerInfo {
        id: "web".into(),
        name: "web".into(),
        image: "nginx".into(),
        status: "running".into(),
        ports: vec![],
        created: "".into(),
    }));

    orchestrate_service(&service, &backend).await.unwrap();

    let calls = backend.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].method, "inspect");
}

#[tokio::test]
async fn test_orchestrate_start_stopped() {
    let backend = MockBackend::new();
    let service = Service {
        name: "web".into(),
        image: Some("nginx".into()),
        container_name: Some("web-container".into()),
        ..Service::new("web".into(), Some("nginx".into()))
    };

    // First inspect: status = "exited"
    backend.push_response(MockResponse::OkInfo(ContainerInfo {
        id: "web-container".into(),
        name: "web-container".into(),
        image: "nginx".into(),
        status: "exited".into(),
        ports: vec![],
        created: "".into(),
    }));
    // Second inspect (exists check): Ok
    backend.push_response(MockResponse::Ok);

    orchestrate_service(&service, &backend).await.unwrap();

    let calls = backend.calls();
    assert!(calls.iter().any(|c| c.method == "start"));
}
