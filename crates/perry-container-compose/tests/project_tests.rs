use perry_container_compose::project::*;
use perry_container_compose::config::*;
use std::fs;
use std::env;

// Feature: perry-container | Layer: unit | Req: 9.1 | Property: -
#[test]
fn test_project_load_discovery() {
    let temp_dir = env::temp_dir().join(format!("test-project-{}", rand::random::<u32>()));
    fs::create_dir_all(&temp_dir).unwrap();
    let compose_path = temp_dir.join("compose.yaml");
    fs::write(&compose_path, "services:\n  web:\n    image: alpine").unwrap();

    let config = ProjectConfig {
        files: vec![compose_path],
        project_name: Some("test-proj".into()),
        env_files: vec![],
    };

    let project = ComposeProject::load(&config).expect("Should load project");
    assert_eq!(project.spec.services.len(), 1);
    assert!(project.spec.services.contains_key("web"));

    fs::remove_dir_all(&temp_dir).unwrap();
}
