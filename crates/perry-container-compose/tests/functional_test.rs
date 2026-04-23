use perry_container_compose::types::{ComposeSpec, ComposeService, DependsOnSpec};
use perry_container_compose::compose::ComposeEngine;
use perry_container_compose::testing::mock_backend::{MockBackend, MockResponse, RecordedCall};
use indexmap::IndexMap;
use std::sync::Arc;

#[tokio::test]
async fn test_compose_up_success() {
    let mut services = IndexMap::new();
    services.insert("db".to_string(), ComposeService {
        image: Some("postgres".to_string()),
        ..Default::default()
    });
    services.insert("api".to_string(), ComposeService {
        image: Some("api-image".to_string()),
        depends_on: Some(DependsOnSpec::List(vec!["db".to_string()])),
        ..Default::default()
    });

    let spec = ComposeSpec { services, ..Default::default() };
    let backend = Arc::new(MockBackend::new());
    let engine = ComposeEngine::new(spec, "test-proj".to_string(), backend.clone());

    let handle = engine.up(true, false, false).await.unwrap();
    assert_eq!(handle.services, vec!["db", "api"]);

    let calls = backend.take_calls();

    let run_calls: Vec<_> = calls.iter().filter_map(|c| {
        if let RecordedCall::Run(spec) = c {
            Some(spec.image.clone())
        } else {
            None
        }
    }).collect();

    assert_eq!(run_calls, vec!["postgres", "api-image"]);
}

#[tokio::test]
async fn test_compose_up_rollback() {
    let mut services = IndexMap::new();
    services.insert("s1".to_string(), ComposeService {
        image: Some("img1".to_string()),
        ..Default::default()
    });
    services.insert("s2".to_string(), ComposeService {
        image: Some("img2".to_string()),
        ..Default::default()
    });

    let spec = ComposeSpec { services, ..Default::default() };
    let backend = Arc::new(MockBackend::new());

    // First service succeeds, second fails
    backend.push_response(MockResponse::Run(Ok("id1".to_string())));
    backend.push_response(MockResponse::Run(Err(perry_container_compose::error::ComposeError::BackendError {
        code: 1,
        message: "fail".to_string()
    })));

    let engine = ComposeEngine::new(spec, "test-proj".to_string(), backend.clone());
    let result = engine.up(true, false, false).await;
    assert!(result.is_err());

    let calls = backend.take_calls();

    // Verify rollback: stop and remove s1 (check if name contains project and service)
    let has_stop = calls.iter().any(|c| matches!(c, RecordedCall::Stop(name, _) if name.contains("test-proj") && name.contains("s1")));
    let has_rm = calls.iter().any(|c| matches!(c, RecordedCall::Remove(name, _) if name.contains("test-proj") && name.contains("s1")));

    assert!(has_stop, "Rollback should have stopped s1 container");
    assert!(has_rm, "Rollback should have removed s1 container");
}
