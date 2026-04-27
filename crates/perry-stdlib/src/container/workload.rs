use std::collections::HashMap;
use serde::{Serialize, Deserialize};
pub use perry_container_compose::workload::*;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MicroVmConfig {
    pub vcpus: u32,
    pub memory_mib: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PolicySpec {
    pub tier: String,
    pub no_network: bool,
    pub read_only_root: bool,
    pub seccomp: bool,
}
