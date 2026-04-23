use perry_container_compose::compose::ComposeEngine;
use perry_container_compose::types::{ComposeSpec, ComposeService, DependsOnSpec};
use perry_container_compose::testing::mock_backend::{MockBackend, MockResponse, RecordedCall};
use std::sync::Arc;
use indexmap::IndexMap;

#[tokio::test]
async fn test_up_creates_networks_before_containers() {
    let mut spec = ComposeSpec::default();
    let mut services = IndexMap::new();
    services.insert("web".into(), ComposeService {
        image: Some("nginx".into()),
        ..Default::default()
    });
    spec.services = services;

    let mut networks = IndexMap::new();
    networks.insert("frontend".into(), None);
    spec.networks = Some(networks);

    let backend = Arc::new(MockBackend::new("mock"));
    let engine = ComposeEngine::new(spec, backend.clone());

    engine.up(false).await.unwrap();

    let calls = backend.take_calls();

    let create_network_idx = calls.iter().position(|c| matches!(c, RecordedCall::CreateNetwork(_, _))).unwrap();
    let run_idx = calls.iter().position(|c| matches!(c, RecordedCall::Run(_))).unwrap();

    assert!(create_network_idx < run_idx, "Network must be created before running container");
}

#[tokio::test]
async fn test_up_starts_services_in_dependency_order() {
    let mut spec = ComposeSpec::default();
    let mut services = IndexMap::new();

    services.insert("db".into(), ComposeService {
        image: Some("postgres".into()),
        ..Default::default()
    });

    services.insert("web".into(), ComposeService {
        image: Some("nginx".into()),
        depends_on: Some(DependsOnSpec::List(vec!["db".into()])),
        ..Default::default()
    });

    spec.services = services;

    let backend = Arc::new(MockBackend::new("mock"));
    let engine = ComposeEngine::new(spec, backend.clone());

    engine.up(false).await.unwrap();

    let calls = backend.take_calls();

    let db_run_idx = calls.iter().position(|c| {
        if let RecordedCall::Run(spec) = c {
            spec.image == "postgres"
        } else { false }
    }).unwrap();

    let web_run_idx = calls.iter().position(|c| {
        if let RecordedCall::Run(spec) = c {
            spec.image == "nginx"
        } else { false }
    }).unwrap();

    assert!(db_run_idx < web_run_idx, "Database must start before web server");
}

#[tokio::test]
async fn test_up_rollback_on_service_failure() {
    let mut spec = ComposeSpec::default();
    let mut services = IndexMap::new();

    services.insert("s1".into(), ComposeService { image: Some("i1".into()), ..Default::default() });
    services.insert("s2".into(), ComposeService { image: Some("i2".into()), ..Default::default() });
    services.insert("s3".into(), ComposeService { image: Some("i3".into()), ..Default::default() });

    spec.services = services;

    let backend = Arc::new(MockBackend::new("mock"));

    // Script:
    backend.push_response(MockResponse::ContainerList(vec![])); // list(true)

    // Script: s1 ok, s2 ok, s3 fails
    backend.push_response(MockResponse::Ok); // pull s1
    backend.push_response(MockResponse::ContainerHandle(perry_container_compose::types::ContainerHandle { id: "h1".into(), name: None })); // run s1
    backend.push_response(MockResponse::Ok); // pull s2
    backend.push_response(MockResponse::ContainerHandle(perry_container_compose::types::ContainerHandle { id: "h2".into(), name: None })); // run s2
    backend.push_response(MockResponse::Ok); // pull s3
    backend.push_response(MockResponse::Error(perry_container_compose::error::ComposeError::ServiceStartupFailed {
        service: "s3".into(),
        message: "failed".into()
    })); // run s3

    let engine = ComposeEngine::new(spec, backend.clone());

    let res = engine.up(false).await;
    assert!(res.is_err());

    let calls = backend.take_calls();

    // Check for rollback: stop and remove h2, then h1
    let stop_h2 = calls.iter().any(|c| matches!(c, RecordedCall::Stop(id, _) if id == "h2"));
    let stop_h1 = calls.iter().any(|c| matches!(c, RecordedCall::Stop(id, _) if id == "h1"));
    let remove_h2 = calls.iter().any(|c| matches!(c, RecordedCall::Remove(id, _) if id == "h2"));
    let remove_h1 = calls.iter().any(|c| matches!(c, RecordedCall::Remove(id, _) if id == "h1"));

    assert!(stop_h2);
    assert!(stop_h1);
    assert!(remove_h2);
    assert!(remove_h1);
}
