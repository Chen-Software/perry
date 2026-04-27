//! Workload graph types.

use std::collections::HashMap;
pub use perry_container_compose::types::{
    ExecutionStrategy, FailureStrategy, GraphStatus, NodeInfo, NodeState, PolicySpec,
    PolicyTier, RefProjection, RunGraphOptions, RuntimeSpec, WorkloadEdge, WorkloadEnvValue,
    WorkloadGraph, WorkloadNode, WorkloadRef,
};

use crate::container::types::ContainerInfo;


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workload_ref_resolution() {
        let db_info = ContainerInfo {
            id: "container-db-123".to_string(),
            name: "db".to_string(),
            image: "postgres".to_string(),
            status: "running".to_string(),
            ports: vec!["5432:5432".to_string()],
            created: "".to_string(),
            labels: HashMap::new(),
            env: HashMap::new(),
            ip_address: "172.17.0.2".to_string(),
        };

        let r = WorkloadRef {
            node_id: "db".to_string(),
            projection: RefProjection::Endpoint,
            port: Some("5432".to_string()),
        };
        assert_eq!(r.resolve(&db_info).unwrap(), "172.17.0.2:5432");

        let r2 = WorkloadRef {
            node_id: "db".to_string(),
            projection: RefProjection::Ip,
            port: None,
        };
        assert_eq!(r2.resolve(&db_info).unwrap(), "172.17.0.2");
    }
}
