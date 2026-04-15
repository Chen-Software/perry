use indexmap::IndexMap;
use crate::error::ComposeError;
use crate::types::{ComposeSpec, ContainerInfo, ContainerLogs};
use crate::backend::ContainerBackend;
use std::collections::BTreeSet;
use std::sync::Arc;

#[derive(Clone)]
pub struct ComposeEngine {
    pub spec: ComposeSpec,
    pub backend: Arc<dyn ContainerBackend + Send + Sync>,
}

impl ComposeEngine {
    pub fn new(spec: ComposeSpec, backend: Arc<dyn ContainerBackend + Send + Sync>) -> Self {
        Self { spec, backend }
    }

    pub fn resolve_startup_order(spec: &ComposeSpec) -> Result<Vec<String>, ComposeError> {
        // 1. Build adjacency list: service → its dependencies
        let mut in_degree: IndexMap<String, usize> = IndexMap::new();
        let mut dependents: IndexMap<String, Vec<String>> = IndexMap::new();

        // Initialize all services with in-degree 0
        for name in spec.services.keys() {
            in_degree.insert(name.clone(), 0);
            dependents.insert(name.clone(), Vec::new());
        }

        // 2. Compute in-degrees from depends_on
        for (name, service) in &spec.services {
            if let Some(deps) = &service.depends_on {
                for dep in deps.service_names() {
                    if !spec.services.contains_key(&dep) {
                        return Err(ComposeError::ValidationError {
                            message: format!("Service '{}' depends on '{}' which is not defined", name, dep)
                        });
                    }
                    // dep must start before name, so name has dep as a prerequisite
                    *in_degree.get_mut(name).unwrap() += 1;
                    dependents.get_mut(&dep).unwrap().push(name.clone());
                }
            }
        }

        // 3. Queue all services with in-degree 0 (sorted for determinism)
        let mut queue: BTreeSet<String> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(name, _)| name.clone())
            .collect();

        // 4. Process queue
        let mut order: Vec<String> = Vec::new();
        while let Some(service) = queue.pop_first() {
            order.push(service.clone());
            for dependent in dependents.get(&service).unwrap_or(&Vec::new()).clone() {
                let deg = in_degree.get_mut(&dependent).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    queue.insert(dependent);
                }
            }
        }

        // 5. If not all services processed → cycle detected
        if order.len() != spec.services.len() {
            let cycle_services: Vec<String> = in_degree
                .iter()
                .filter(|(_, &deg)| deg > 0)
                .map(|(name, _)| name.clone())
                .collect();
            return Err(ComposeError::DependencyCycle { services: cycle_services });
        }

        Ok(order)
    }

    pub async fn up(&self) -> Result<(), ComposeError> {
        let _order = Self::resolve_startup_order(&self.spec)?;
        // Real implementation would iterate and call backend.run()
        Ok(())
    }

    pub async fn down(&self, _volumes: bool) -> Result<(), ComposeError> {
        Ok(())
    }

    pub async fn ps(&self) -> Result<Vec<ContainerInfo>, ComposeError> {
        Ok(vec![])
    }

    pub async fn logs(&self, _service: Option<&str>, _tail: Option<u32>) -> Result<ContainerLogs, ComposeError> {
        Ok(ContainerLogs { stdout: "".into(), stderr: "".into() })
    }

    pub async fn exec(&self, _service: &str, _cmd: &[String]) -> Result<ContainerLogs, ComposeError> {
        Ok(ContainerLogs { stdout: "".into(), stderr: "".into() })
    }

    pub async fn start(&self, _services: &[String]) -> Result<(), ComposeError> {
        Ok(())
    }

    pub async fn stop(&self, _services: &[String]) -> Result<(), ComposeError> {
        Ok(())
    }

    pub async fn restart(&self, _services: &[String]) -> Result<(), ComposeError> {
        Ok(())
    }
}
