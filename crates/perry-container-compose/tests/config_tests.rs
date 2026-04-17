use perry_container_compose::config::*;
use std::env;

// Feature: perry-container | Layer: unit | Req: 9.5 | Property: -
#[test]
fn test_resolve_project_name_explicit() {
    let name = resolve_project_name(Some("my-proj")).unwrap();
    assert_eq!(name, "my-proj");
}

// Feature: perry-container | Layer: unit | Req: 9.5 | Property: -
#[test]
fn test_resolve_project_name_env() {
    env::set_var("COMPOSE_PROJECT_NAME", "env-proj");
    let name = resolve_project_name(None).unwrap();
    assert_eq!(name, "env-proj");
    env::remove_var("COMPOSE_PROJECT_NAME");
}

// Feature: perry-container | Layer: unit | Req: 9.1 | Property: -
#[test]
fn test_resolve_compose_files_explicit() {
    let files = vec!["custom.yml".into()];
    let resolved = resolve_compose_files(&files).unwrap();
    assert_eq!(resolved, files);
}

// Feature: perry-container | Layer: unit | Req: 9.1 | Property: -
#[test]
fn test_resolve_compose_files_env() {
    env::set_var("COMPOSE_FILE", "f1.yml:f2.yml");
    let resolved = resolve_compose_files(&[]).unwrap();
    assert_eq!(resolved.len(), 2);
    assert_eq!(resolved[0].to_str().unwrap(), "f1.yml");
    assert_eq!(resolved[1].to_str().unwrap(), "f2.yml");
    env::remove_var("COMPOSE_FILE");
}
