use perry_container_compose::compose::ComposeEngine;
use perry_container_compose::testing::mock_backend::{MockBackend, RecordedCall};
use perry_container_compose::types::{ComposeSpec, ComposeService};
use indexmap::IndexMap;
use std::sync::Arc;

#[tokio::test]
async fn test_up_starts_services_in_dependency_order() {
    let mut services = IndexMap::new();

    let mut db = ComposeService::default();
    db.image = Some("postgres".to_string());
    services.insert("db".to_string(), db);

    let mut api = ComposeService::default();
    api.image = Some("api".to_string());
    api.depends_on = Some(perry_container_compose::types::DependsOnSpec::List(vec!["db".to_string()]));
    services.insert("api".to_string(), api);

    let spec = ComposeSpec {
        services,
        ..Default::default()
    };

    let backend = Arc::new(MockBackend::new("mock"));
    // Return NotFound for both services to trigger Run
    backend.responses.lock().unwrap().push_back(Err(perry_container_compose::error::ComposeError::NotFound("db".to_string())));
    backend.responses.lock().unwrap().push_back(Err(perry_container_compose::error::ComposeError::NotFound("api".to_string())));

    let engine = Arc::new(ComposeEngine::new(spec, "test-proj".to_string(), backend.clone()));

    engine.up(&[], false, false, false).await.unwrap();

    let calls = backend.calls.lock().unwrap();
    println!("Recorded calls: {:?}", *calls);
    let run_calls: Vec<_> = calls.iter().filter_map(|c| match c {
        RecordedCall::Run(spec) => Some(spec.image.clone()),
        _ => None,
    }).collect();

    assert_eq!(run_calls.len(), 2);
    assert_eq!(run_calls[0], "postgres");
    assert_eq!(run_calls[1], "api");
}
