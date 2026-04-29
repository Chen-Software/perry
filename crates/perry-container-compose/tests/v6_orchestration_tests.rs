//! V6 Orchestration tests: Log separation and Orphan removal.
//! Hermetic tests using MockBackend.

#![cfg(feature = "test-utils")]

use perry_container_compose::backend::ContainerBackend;
use perry_container_compose::compose::ComposeEngine;
use perry_container_compose::testing::mock_backend::{MockBackend, RecordedCall};
use perry_container_compose::types::{ComposeService, ComposeSpec, ContainerInfo};
use std::collections::HashMap;
use std::sync::Arc;

fn svc(image: &str) -> ComposeService {
    ComposeService {
        image: Some(image.to_string()),
        ..Default::default()
    }
}

#[tokio::test]
async fn test_orphan_removal_on_up() {
    let mock = Arc::new(MockBackend::new());

    // Setup: Mock returns one "legitimate" container and one "orphan"
    // legitimate: project="myapp", service="web"
    // orphan:     project="myapp", service="old-svc" (not in spec)
    let mut labels_legit = HashMap::new();
    labels_legit.insert("perry.compose.project".to_string(), "myapp".to_string());
    labels_legit.insert("perry.compose.service".to_string(), "web".to_string());

    let mut labels_orphan = HashMap::new();
    labels_orphan.insert("perry.compose.project".to_string(), "myapp".to_string());
    labels_orphan.insert("perry.compose.service".to_string(), "old-svc".to_string());

    let containers = vec![
        ContainerInfo {
            id: "web-id".to_string(),
            name: "myapp-web".to_string(),
            image: "nginx".to_string(),
            status: "running".to_string(),
            ports: vec![],
            labels: labels_legit,
            created: "".to_string(),
            ip_address: "".to_string(),
        },
        ContainerInfo {
            id: "orphan-id".to_string(),
            name: "myapp-old".to_string(),
            image: "redis".to_string(),
            status: "running".to_string(),
            ports: vec![],
            labels: labels_orphan,
            created: "".to_string(),
            ip_address: "".to_string(),
        }
    ];
    mock.set_list_result(containers).await;
    mock.set_inspect_running(true).await;

    // Spec only contains "web"
    let mut services = indexmap::IndexMap::new();
    services.insert("web".to_string(), svc("nginx"));
    let spec = ComposeSpec { services, ..Default::default() };

    let eng = Arc::new(ComposeEngine::new(spec, "myapp".to_string(), mock.clone() as Arc<dyn ContainerBackend>));

    // up() with remove_orphans=true
    let _ = eng.up(&[], false, false, true).await.expect("up failed");

    let calls = mock.calls().await;

    // Verify orphan-id was stopped and removed
    let stopped_orphan = calls.iter().any(|c| matches!(c, RecordedCall::Stop(id, _) if id == "orphan-id"));
    let removed_orphan = calls.iter().any(|c| matches!(c, RecordedCall::Remove(id, _) if id == "orphan-id"));

    assert!(stopped_orphan, "orphan container should be stopped");
    assert!(removed_orphan, "orphan container should be removed");

    // Verify web-id was NOT removed (it's in the spec and already running/matching spec hash)
    let removed_web = calls.iter().any(|c| matches!(c, RecordedCall::Remove(id, _) if id == "web-id"));
    assert!(!removed_web, "legitimate service container should NOT be removed");
}

#[tokio::test]
async fn test_structured_log_aggregation() {
    let mock = Arc::new(MockBackend::new());

    // up() first to populate cache
    let mut services = indexmap::IndexMap::new();
    services.insert("web".to_string(), svc("nginx"));
    services.insert("api".to_string(), svc("node"));
    let spec = ComposeSpec { services, ..Default::default() };

    mock.set_inspect_not_found().await;
    let eng = Arc::new(ComposeEngine::new(spec, "myapp".to_string(), mock.clone() as Arc<dyn ContainerBackend>));
    let _ = eng.clone().up(&[], false, false, false).await.expect("up failed");

    // Mock logs for each service
    // MockBackend uses a simple logs implementation that returns fixed strings.
    // In our case, it returns "web stdout" / "web stderr" based on service name if we had that logic.
    // Actually the mock returns the same for all. Let's just verify we get a map of ContainerLogs.

    let logs_map = eng.logs(&[], None).await.expect("logs failed");

    assert_eq!(logs_map.len(), 2);
    assert!(logs_map.contains_key("web"));
    assert!(logs_map.contains_key("api"));

    // Verify we got ContainerLogs objects (which have stdout/stderr fields)
    let web_logs = &logs_map["web"];
    assert!(web_logs.stdout.contains("mock stdout"));
    assert!(web_logs.stderr.contains("mock stderr"));
}
