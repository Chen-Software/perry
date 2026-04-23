use perry_container_compose::workload::*;
use perry_container_compose::testing::mock_backend::{MockBackend, RecordedCall};
use std::sync::Arc;
use indexmap::IndexMap;
use std::collections::HashMap;

#[tokio::test]
async fn test_workload_run_sequential() {
    let mut graph = WorkloadGraph {
        name: "test".into(),
        nodes: IndexMap::new(),
        edges: vec![],
    };

    graph.nodes.insert("n1".into(), WorkloadNode {
        id: "n1".into(),
        name: "node1".into(),
        image: Some("alpine".into()),
        resources: None,
        ports: vec![],
        env: HashMap::new(),
        depends_on: vec![],
        runtime: RuntimeSpec::Auto,
        policy: PolicySpec { tier: PolicyTier::Default, no_network: None, read_only_root: None, seccomp: None },
    });

    graph.nodes.insert("n2".into(), WorkloadNode {
        id: "n2".into(),
        name: "node2".into(),
        image: Some("alpine".into()),
        resources: None,
        ports: vec![],
        env: HashMap::new(),
        depends_on: vec!["n1".into()],
        runtime: RuntimeSpec::Auto,
        policy: PolicySpec { tier: PolicyTier::Default, no_network: None, read_only_root: None, seccomp: None },
    });

    graph.edges.push(WorkloadEdge { from: "n1".into(), to: "n2".into() });

    let backend = Arc::new(MockBackend::new("mock"));
    let engine = WorkloadGraphEngine::new(graph, backend.clone());

    let opts = RunGraphOptions {
        strategy: Some(ExecutionStrategy::Sequential),
        on_failure: None,
    };

    let handle = engine.run(opts).await.unwrap();
    assert_eq!(handle.started_nodes.len(), 2);

    let calls = backend.take_calls();
    let n1_idx = calls.iter().position(|c| matches!(c, RecordedCall::Run(spec) if spec.name == Some("node1".into()))).unwrap();
    let n2_idx = calls.iter().position(|c| matches!(c, RecordedCall::Run(spec) if spec.name == Some("node2".into()))).unwrap();
    assert!(n1_idx < n2_idx);
}

#[tokio::test]
async fn test_workload_rollback_on_failure() {
    let mut graph = WorkloadGraph {
        name: "test".into(),
        nodes: IndexMap::new(),
        edges: vec![],
    };

    graph.nodes.insert("n1".into(), WorkloadNode {
        id: "n1".into(),
        name: "node1".into(),
        image: Some("alpine".into()),
        resources: None,
        ports: vec![],
        env: HashMap::new(),
        depends_on: vec![],
        runtime: RuntimeSpec::Auto,
        policy: PolicySpec { tier: PolicyTier::Default, no_network: None, read_only_root: None, seccomp: None },
    });

    graph.nodes.insert("n2".into(), WorkloadNode {
        id: "n2".into(),
        name: "node2".into(),
        image: Some("fail".into()),
        resources: None,
        ports: vec![],
        env: HashMap::new(),
        depends_on: vec!["n1".into()],
        runtime: RuntimeSpec::Auto,
        policy: PolicySpec { tier: PolicyTier::Default, no_network: None, read_only_root: None, seccomp: None },
    });

    graph.edges.push(WorkloadEdge { from: "n1".into(), to: "n2".into() });

    let backend = Arc::new(MockBackend::new("mock"));

    // Script: n1 ok, n2 fails
    backend.push_response(perry_container_compose::testing::mock_backend::MockResponse::ContainerHandle(perry_container_compose::types::ContainerHandle { id: "h1".into(), name: Some("node1".into()) }));
    backend.push_response(perry_container_compose::testing::mock_backend::MockResponse::Error(perry_container_compose::error::ComposeError::BackendError { code: 1, message: "fail".into() }));

    let engine = WorkloadGraphEngine::new(graph, backend.clone());

    let opts = RunGraphOptions {
        strategy: Some(ExecutionStrategy::Sequential),
        on_failure: Some(FailureStrategy::RollbackAll),
    };

    let res = engine.run(opts).await;
    assert!(res.is_err());

    let calls = backend.take_calls();
    // Should see stop/remove for n1 (h1)
    let stop = calls.iter().any(|c| matches!(c, RecordedCall::Stop(id, _) if id == "h1"));
    let remove = calls.iter().any(|c| matches!(c, RecordedCall::Remove(id, _) if id == "h1"));
    assert!(stop);
    assert!(remove);
}
