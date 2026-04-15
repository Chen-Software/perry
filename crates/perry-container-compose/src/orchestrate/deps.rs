//! Dependency resolution — topological sort of service `depends_on` graph.
//!
//! Implements DFS-based topological sort with cycle detection.

use crate::entities::compose::Compose;
use crate::error::{ComposeError, Result};
use std::collections::{HashMap, HashSet};

/// Perform a topological sort of the services in a compose spec.
///
/// Returns an ordered list of service names where each service appears
/// *after* all of its dependencies.
pub fn topological_order(compose: &Compose) -> Result<Vec<String>> {
    let mut result: Vec<String> = Vec::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut visiting: HashSet<String> = HashSet::new(); // currently on the DFS stack

    // Build adjacency list: service → its dependencies
    let mut deps: HashMap<String, Vec<String>> = HashMap::new();
    for (name, svc) in &compose.services {
        let dep_names = svc
            .depends_on
            .as_ref()
            .map(|d| d.service_names())
            .unwrap_or_default();

        // Validate that all dependencies exist
        for dep in &dep_names {
            if !compose.services.contains_key(dep) {
                return Err(ComposeError::validation(format!(
                    "Service '{}' depends on '{}', which is not defined in the compose file",
                    name, dep
                )));
            }
        }

        deps.insert(name.clone(), dep_names);
    }

    // Iterate in deterministic order for reproducibility
    let mut names: Vec<String> = compose.services.keys().cloned().collect();
    names.sort();

    for name in &names {
        if !visited.contains(name) {
            dfs(name, &deps, &mut visited, &mut visiting, &mut result)?;
        }
    }

    Ok(result)
}

fn dfs(
    node: &str,
    deps: &HashMap<String, Vec<String>>,
    visited: &mut HashSet<String>,
    visiting: &mut HashSet<String>,
    result: &mut Vec<String>,
) -> Result<()> {
    visiting.insert(node.to_owned());

    if let Some(neighbors) = deps.get(node) {
        for dep in neighbors {
            if visiting.contains(dep) {
                return Err(ComposeError::CircularDependency {
                    cycle: format!("{} -> {}", node, dep),
                });
            }
            if !visited.contains(dep) {
                dfs(dep, deps, visited, visiting, result)?;
            }
        }
    }

    visiting.remove(node);
    visited.insert(node.to_owned());
    result.push(node.to_owned());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::{compose::Compose, service::Service};
    use crate::entities::service::{DependsOn};
    use std::collections::HashMap;

    fn make_compose(edges: &[(&str, &[&str])]) -> Compose {
        let mut services = HashMap::new();
        for (name, deps) in edges {
            let mut svc = Service::default();
            if !deps.is_empty() {
                svc.depends_on = Some(DependsOn::List(
                    deps.iter().map(|s| s.to_string()).collect(),
                ));
            }
            services.insert(name.to_string(), svc);
        }
        Compose {
            services,
            ..Default::default()
        }
    }

    #[test]
    fn test_simple_chain() {
        // db → web → proxy
        let compose = make_compose(&[("web", &["db"]), ("db", &[]), ("proxy", &["web"])]);
        let order = topological_order(&compose).unwrap();
        // db must come before web, web before proxy
        let pos = |name: &str| order.iter().position(|s| s == name).unwrap();
        assert!(pos("db") < pos("web"), "db must precede web");
        assert!(pos("web") < pos("proxy"), "web must precede proxy");
    }

    #[test]
    fn test_no_deps() {
        let compose = make_compose(&[("a", &[]), ("b", &[]), ("c", &[])]);
        let order = topological_order(&compose).unwrap();
        assert_eq!(order.len(), 3);
    }

    #[test]
    fn test_cycle_detected() {
        let compose = make_compose(&[("a", &["b"]), ("b", &["a"])]);
        let result = topological_order(&compose);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ComposeError::CircularDependency { .. }));
    }
}
