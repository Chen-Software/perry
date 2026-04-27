use serde::{Deserialize, Serialize};
use crate::container::types::ContainerSpec;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorkloadGraph {
    pub name: String,
    pub nodes: Vec<WorkloadNode>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorkloadNode {
    pub id: String,
    pub spec: ContainerSpec,
    pub depends_on: Vec<String>,
}
