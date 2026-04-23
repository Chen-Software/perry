use perry_container_compose::compose::ComposeEngine;
use perry_container_compose::types::ComposeSpec;
use perry_container_compose::testing::MockBackend;
use std::sync::Arc;

#[tokio::test]
async fn test_up_starts_in_order() {
    let yaml = include_str!("../fixtures/simple-two-service.yaml");
    let spec = ComposeSpec::parse_str(yaml).unwrap();
    let mock = Arc::new(MockBackend::new());
    let engine = ComposeEngine::new(spec, "test".to_string(), mock.clone());

    engine.up(&[], true, false, false).await.unwrap();

    let calls = mock.calls();
    // Should see 'db' run before 'web' due to depends_on
    let db_idx = calls.iter().position(|c| c.method == "run" && c.args[0].contains("db")).unwrap();
    let web_idx = calls.iter().position(|c| c.method == "run" && c.args[0].contains("web")).unwrap();
    assert!(db_idx < web_idx);
}
