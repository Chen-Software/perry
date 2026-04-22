use perry_container_compose::compose::ComposeEngine;
use perry_container_compose::types::ContainerCompose;
use perry_container_compose::testing::mock_backend::{MockBackend, RecordedCall};
use std::sync::Arc;

#[tokio::test]
async fn test_up_starts_services_in_dependency_order() {
    let mut spec = ContainerCompose::default();
    spec.services.insert("db".into(), Default::default());
    let mut api = perry_container_compose::types::ComposeService::default();
    api.depends_on = Some(perry_container_compose::types::DependsOnSpec::List(vec!["db".into()]));
    spec.services.insert("api".into(), api);

    let backend = Arc::new(MockBackend::new());
    let engine = ComposeEngine::new(spec, "test".into(), backend.clone());

    let _ = engine.up(&[], false, false, false).await.unwrap();

    let calls = backend.calls.lock().unwrap();
    let run_calls: Vec<_> = calls.iter().filter_map(|c| match c {
        RecordedCall::Run(spec) => Some(spec.name.clone().unwrap()),
        _ => None,
    }).collect();

    assert_eq!(run_calls.len(), 2);
    assert_eq!(run_calls[0], "db");
    assert_eq!(run_calls[1], "api");
}
