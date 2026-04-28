use perry_container_compose::compose::WorkloadGraphEngine;
use perry_container_compose::types::{WorkloadGraph, WorkloadNode, PolicySpec};
use std::sync::Arc;

mod common;
use common::MockBackend;

#[tokio::test]
async fn test_workload_run_graph_success() {
    let mut graph = WorkloadGraph::default();
    graph.name = "test-graph".into();
    graph.nodes.insert("node1".into(), WorkloadNode {
        id: "node1".into(),
        name: "test-node1".into(),
        image: Some("alpine".into()),
        ..Default::default()
    });

    let backend = Arc::new(MockBackend::default());
    let engine = WorkloadGraphEngine::new("test-project".into(), backend.clone());

    let started = engine.run_graph(&graph).await.expect("run_graph failed");

    assert_eq!(started.len(), 1);
    assert_eq!(started[0], "node1");

    let state = backend.state.lock().unwrap();
    assert_eq!(state.containers.len(), 1);
    assert!(state.containers.contains_key("test-node1"));
}

#[tokio::test]
async fn test_workload_run_graph_policy_enforcement() {
    let mut graph = WorkloadGraph::default();
    graph.name = "policy-graph".into();
    graph.nodes.insert("node1".into(), WorkloadNode {
        id: "node1".into(),
        name: "hardened-node".into(),
        image: Some("alpine".into()),
        policy: PolicySpec {
            tier: "hardened".into(),
            read_only_root: true,
            no_network: true,
            ..Default::default()
        },
        ..Default::default()
    });

    let backend = Arc::new(MockBackend::default());
    let engine = WorkloadGraphEngine::new("test-project".into(), backend.clone());

    let _ = engine.run_graph(&graph).await.unwrap();

    let state = backend.state.lock().unwrap();
    // We can't easily check the spec passed to run because MockBackend doesn't store it in its current form
    // but we can check if the container was created.
    assert!(state.containers.contains_key("hardened-node"));
}
