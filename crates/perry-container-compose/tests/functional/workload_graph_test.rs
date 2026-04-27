use perry_container_compose::compose::WorkloadGraphEngine;
use perry_container_compose::testing::mock_backend::{MockBackend, RecordedCall};
use perry_container_compose::types::{WorkloadGraph, WorkloadNode, RuntimeSpec, PolicySpec, PolicyTier, RunGraphOptions, ExecutionStrategy, FailureStrategy};
use indexmap::IndexMap;
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::test]
async fn test_workload_run_applies_policy() {
    let mut nodes = IndexMap::new();

    let db = WorkloadNode {
        id: "db".to_string(),
        name: "db".to_string(),
        image: Some("postgres".to_string()),
        resources: None,
        ports: vec![],
        env: HashMap::new(),
        depends_on: vec![],
        runtime: RuntimeSpec::Auto,
        policy: PolicySpec {
            tier: PolicyTier::Hardened,
            no_network: false,
            read_only_root: false,
            seccomp: false,
        },
    };
    nodes.insert("db".to_string(), db);

    let graph = WorkloadGraph {
        name: "test-graph".to_string(),
        nodes,
        edges: vec![],
    };

    let backend = Arc::new(MockBackend::new("mock"));
    backend.responses.lock().unwrap().push_back(Err(perry_container_compose::error::ComposeError::NotFound("db".to_string())));

    let engine = WorkloadGraphEngine::new(backend.clone(), "test-proj".to_string());
    let opts = RunGraphOptions {
        strategy: ExecutionStrategy::DependencyAware,
        on_failure: FailureStrategy::RollbackAll,
    };

    engine.run(graph, opts).await.unwrap();

    let calls = backend.calls.lock().unwrap();
    let run_spec = calls.iter().find_map(|c| match c {
        RecordedCall::Run(spec) => Some(spec),
        _ => None,
    }).unwrap();

    // Hardened policy should set read_only: true
    assert_eq!(run_spec.read_only, Some(true));
}
