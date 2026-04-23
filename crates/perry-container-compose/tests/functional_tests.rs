use perry_container_compose::testing::mock_backend::{MockBackend, MockResponse};
use perry_container_compose::compose::ComposeEngine;
use perry_container_compose::types::{ComposeSpec, ComposeService, ContainerHandle, ContainerInfo, DependsOnSpec};
use std::sync::Arc;

#[tokio::test]
async fn test_compose_up_simple() {
    let mock = Arc::new(MockBackend::new());
    let mut spec = ComposeSpec::default();
    spec.services.insert("web".into(), Default::default());

    let engine = ComposeEngine::new(spec, "test".into(), mock.clone());

    mock.push_response(MockResponse::List(vec![]));
    mock.push_response(MockResponse::Run(ContainerHandle { id: "web_id".into(), name: Some("web_name".into()) }));

    let res = engine.up(&[], false, false, false).await;
    assert!(res.is_ok());

    let calls = mock.calls.lock().unwrap();
    assert_eq!(calls[0], "list");
    assert_eq!(calls[1], "pull_image");
    assert_eq!(calls[2], "run");
}

#[tokio::test]
async fn test_compose_up_with_dependency() {
    let mock = Arc::new(MockBackend::new());
    let mut spec = ComposeSpec::default();

    spec.services.insert("db".into(), Default::default());

    let mut web = ComposeService::default();
    web.depends_on = Some(DependsOnSpec::List(vec!["db".into()]));
    spec.services.insert("web".into(), web);

    let engine = ComposeEngine::new(spec, "test".into(), mock.clone());

    // Calls for db: list, run
    mock.push_response(MockResponse::List(vec![]));
    mock.push_response(MockResponse::Run(ContainerHandle { id: "db_id".into(), name: Some("db_name".into()) }));

    // Calls for web: list, run
    mock.push_response(MockResponse::List(vec![
        ContainerInfo { id: "db_id".into(), name: "db_name".into(), image: "db_img".into(), status: "running".into(), ports: vec![], labels: Default::default(), created: "".into() }
    ]));
    mock.push_response(MockResponse::Run(ContainerHandle { id: "web_id".into(), name: Some("web_name".into()) }));

    let res = engine.up(&[], false, false, false).await;
    assert!(res.is_ok());

    let calls = mock.calls.lock().unwrap();
    // Kahn's algorithm will start 'db' then 'web'
    // Each service: list -> pull -> run
    assert_eq!(calls.len(), 6);
    assert_eq!(calls[0], "list");
    assert_eq!(calls[1], "pull_image");
    assert_eq!(calls[2], "run");
    assert_eq!(calls[3], "list");
    assert_eq!(calls[4], "pull_image");
    assert_eq!(calls[5], "run");
}
