use perry_container_compose::config::*;
use std::env;
use std::collections::HashMap;
use std::path::Path;

// Feature: perry-container | Layer: unit | Req: 9.5 | Property: -
#[test]
fn test_resolve_project_name_explicit() {
    let env_map = HashMap::new();
    let name = resolve_project_name(Some("my-proj"), Path::new("."), &env_map);
    assert_eq!(name, "my-proj");
}

// Feature: perry-container | Layer: unit | Req: 9.5 | Property: -
#[test]
fn test_resolve_project_name_env() {
    let mut env_map = HashMap::new();
    env_map.insert("COMPOSE_PROJECT_NAME".to_string(), "env-proj".to_string());
    let name = resolve_project_name(None, Path::new("."), &env_map);
    assert_eq!(name, "env-proj");
}

// Feature: perry-container | Layer: unit | Req: 9.1 | Property: -
#[test]
fn test_resolve_compose_files_explicit() {
    let files = vec!["custom.yml".into()];
    let env_map = HashMap::new();
    let resolved = resolve_compose_files(&files, &env_map);
    assert_eq!(resolved, files);
}

// Feature: perry-container | Layer: unit | Req: 9.1 | Property: -
#[test]
fn test_resolve_compose_files_env() {
    let mut env_map = HashMap::new();
    env_map.insert("COMPOSE_FILE".to_string(), "f1.yml:f2.yml".to_string());
    let resolved = resolve_compose_files(&[], &env_map);
    assert_eq!(resolved.len(), 2);
    assert_eq!(resolved[0].to_str().unwrap(), "f1.yml");
    assert_eq!(resolved[1].to_str().unwrap(), "f2.yml");
}
