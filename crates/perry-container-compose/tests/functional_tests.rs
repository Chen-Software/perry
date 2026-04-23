use perry_container_compose::compose::ComposeEngine;
use perry_container_compose::types::{ComposeSpec, ComposeService, DependsOnSpec};
use perry_container_compose::testing::mock_backend::{MockBackend, RecordedCall};
use std::sync::Arc;

#[tokio::test]
async fn test_compose_engine_up_ordering() {
    let mut spec = ComposeSpec::default();

    let mut db = ComposeService::default();
    db.image = Some("postgres".into());

    let mut api = ComposeService::default();
    api.image = Some("node".into());
    api.depends_on = Some(DependsOnSpec::List(vec!["db".into()]));

    spec.services.insert("db".into(), db);
    spec.services.insert("api".into(), api);

    let backend = Arc::new(MockBackend::new());
    let engine = ComposeEngine::new(spec, Arc::clone(&backend) as Arc<dyn perry_container_compose::backend::ContainerBackend + Send + Sync>);

    let _handle = engine.up().await.unwrap();

    let calls = backend.calls();
    for (i, call) in calls.iter().enumerate() {
        println!("{}: {:?}", i, call);
    }
    let run_db_idx = calls.iter().position(|c| matches!(c, RecordedCall::Run(s) if s.name.as_deref().map_or(false, |n| n.contains("db")))).expect("db run call not found");
    let run_api_idx = calls.iter().position(|c| matches!(c, RecordedCall::Run(s) if s.name.as_deref().map_or(false, |n| n.contains("api")))).expect("api run call not found");

    assert!(run_db_idx < run_api_idx, "db should start before api");
}

#[tokio::test]
async fn test_workload_graph_engine_run() {
    use perry_container_compose::workload::{WorkloadGraph, WorkloadNode, PolicySpec, RuntimeSpec, RunGraphOptions};
    use perry_container_compose::compose::WorkloadGraphEngine;
    use std::collections::HashMap;

    let mut nodes = HashMap::new();
    nodes.insert("db".into(), WorkloadNode {
        id: "db".into(),
        name: "db".into(),
        image: Some("postgres".into()),
        resources: None,
        ports: vec!["5432:5432".into()],
        env: HashMap::new(),
        depends_on: vec![],
        runtime: RuntimeSpec::Auto,
        policy: PolicySpec {
            tier: "default".into(),
            no_network: None,
            read_only_root: None,
            seccomp: None,
        },
    });

    let graph = WorkloadGraph {
        name: "test-graph".into(),
        nodes,
        edges: vec![],
    };

    let backend = Arc::new(MockBackend::new());
    let engine = WorkloadGraphEngine::new(ComposeSpec::default(), Arc::clone(&backend) as Arc<dyn perry_container_compose::backend::ContainerBackend + Send + Sync>);

    let _id = engine.run(graph, RunGraphOptions { strategy: None, on_failure: None }).await.unwrap();

    let calls = backend.calls();
    assert!(calls.iter().any(|c| matches!(c, RecordedCall::Run(s) if s.image == "postgres")), "should run postgres container");
}
