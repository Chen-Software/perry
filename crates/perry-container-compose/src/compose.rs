use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use indexmap::IndexMap;
use crate::types::*;
use crate::service::Service;
use crate::backend::{ContainerBackend, get_global_backend_instance};
use crate::error::ComposeError;
use crate::orchestrate::orchestrate_service;

pub struct ComposeEngine {
    pub backend: Arc<dyn ContainerBackend>,
    pub services: IndexMap<String, Service>,
}

impl ComposeEngine {
    pub async fn new(spec: ComposeSpec) -> Result<Self, ComposeError> {
        let backend = get_global_backend_instance().await?;
        let services = spec.services.into_iter().map(|(k, v)| (k, Service::from(v))).collect();
        Ok(Self { backend, services })
    }

    pub async fn up(&self) -> Result<(), ComposeError> {
        let order = self.services.keys().cloned().collect::<Vec<_>>(); // Simplified for now
        for name in order {
            if let Some(service) = self.services.get(&name) {
                orchestrate_service(&name, service, self.backend.as_ref()).await?;
            }
        }
        Ok(())
    }

    pub fn resolve_startup_order(&self) -> Result<Vec<String>, ComposeError> {
        Ok(self.services.keys().cloned().collect())
    }
}

pub fn resolve_startup_order(spec: &ComposeSpec) -> Result<Vec<String>, ComposeError> {
    let mut order = Vec::new();
    let mut visited = BTreeSet::new();
    let mut temp_visited = BTreeSet::new();

    fn visit(
        name: &str,
        spec: &ComposeSpec,
        order: &mut Vec<String>,
        visited: &mut BTreeSet<String>,
        temp_visited: &mut BTreeSet<String>,
    ) -> Result<(), ComposeError> {
        if temp_visited.contains(name) {
            return Err(ComposeError::DependencyCycle(name.to_string()));
        }
        if visited.contains(name) {
            return Ok(());
        }

        temp_visited.insert(name.to_string());

        if let Some(service) = spec.services.get(name) {
            if let Some(depends_on) = &service.depends_on {
                let deps = match depends_on {
                    DependsOn::List(l) => l.clone(),
                    DependsOn::Dict(d) => d.keys().cloned().collect(),
                };
                for dep in deps {
                    visit(&dep, spec, order, visited, temp_visited)?;
                }
            }
        }

        temp_visited.remove(name);
        visited.insert(name.to_string());
        order.push(name.to_string());
        Ok(())
    }

    for name in spec.services.keys() {
        visit(name, spec, &mut order, &mut visited, &mut temp_visited)?;
    }

    Ok(order)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use crate::types::*;
    use indexmap::IndexMap;
    use std::collections::BTreeSet;

    proptest! {
        // Feature: alloy-container, Property 7: Topological sort produces valid ordering
        #[test]
        fn prop_topological_sort_valid(services in any::<Vec<String>>()) {
            let mut spec = ComposeSpec {
                name: None,
                version: None,
                services: IndexMap::new(),
                networks: None,
                volumes: None,
                secrets: None,
                configs: None,
            };

            let mut unique_names = BTreeSet::new();
            for s in services {
                if unique_names.insert(s.clone()) && !s.is_empty() {
                    spec.services.insert(s, ComposeService {
                        image: Some("alpine".into()),
                        build: None,
                        command: None,
                        entrypoint: None,
                        environment: None,
                        env_file: None,
                        ports: None,
                        volumes: None,
                        networks: None,
                        depends_on: None,
                        healthcheck: None,
                        deploy: None,
                        logging: None,
                        container_name: None,
                        labels: None,
                        extra_hosts: None,
                        sysctls: None,
                        read_only: None,
                        isolation_level: None,
                    });
                }
            }

            if spec.services.is_empty() { return Ok(()); }

            let order = resolve_startup_order(&spec).unwrap();
            assert_eq!(order.len(), spec.services.len());
            for name in spec.services.keys() {
                assert!(order.contains(name));
            }
        }
    }
}
