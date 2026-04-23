//! Workload graph execution engine.

use crate::backend::ContainerBackend;
use crate::error::Result;
use crate::types::{WorkloadGraph, RunGraphOptions, StackStatus};
use std::sync::Arc;

pub struct WorkloadGraphEngine {
    pub backend: Arc<dyn ContainerBackend>,
}

impl WorkloadGraphEngine {
    pub fn new(backend: Arc<dyn ContainerBackend>) -> Self {
        Self { backend }
    }

    pub async fn run(&self, _graph_json: &str, _opts_json: &str) -> Result<()> {
        // Minimum-viable stub to satisfy stdlib FFI
        Ok(())
    }

    pub async fn status(&self, _graph_json: &str) -> Result<StackStatus> {
        // Minimum-viable stub
        Ok(StackStatus {
            services: Vec::new(),
            healthy: true,
        })
    }
}
