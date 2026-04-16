// Feature: perry-container | Layer: unit | Req: 9.1 | Property: -

#[cfg(test)]
mod tests {
    use perry_container_compose::project::ComposeProject;
    use perry_container_compose::config::ProjectConfig;
    use perry_container_compose::error::ComposeError;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_project_load_no_files_error() {
        let config = ProjectConfig::new(vec![], None, vec![]);
        // load() checks CWD if no files provided. If no compose.yaml in CWD, returns error.
        // We run in a fresh temp dir to ensure no default files are found.
        let dir = tempdir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let result = ComposeProject::load(&config);
        match result {
            Err(ComposeError::FileNotFound { path }) => {
                assert!(path.contains("compose.yaml"));
            }
            _ => panic!("Expected FileNotFound error, got {:?}", result),
        }
    }

    #[test]
    fn test_project_load_success() {
        let dir = tempdir().unwrap();
        let compose_path = dir.path().join("compose.yaml");
        fs::write(&compose_path, "services:\n  app:\n    image: node").unwrap();

        std::env::set_current_dir(&dir).unwrap();

        let config = ProjectConfig::new(vec![compose_path], Some("test-project".into()), vec![]);
        let project = ComposeProject::load(&config).expect("Load failed");

        assert_eq!(project.project_name, "test-project");
        assert!(project.spec.services.contains_key("app"));
    }

    #[test]
    fn test_project_load_interpolation_from_env() {
        let dir = tempdir().unwrap();
        let compose_path = dir.path().join("compose.yaml");
        fs::write(&compose_path, "services:\n  app:\n    image: ${IMG}").unwrap();

        std::env::set_current_dir(&dir).unwrap();
        std::env::set_var("IMG", "redis");

        let config = ProjectConfig::new(vec![compose_path], None, vec![]);
        let project = ComposeProject::load(&config).expect("Load failed");

        assert_eq!(project.spec.services["app"].image.as_deref(), Some("redis"));
        std::env::remove_var("IMG");
    }
}

/*
| Requirement | Test name | Layer |
|-------------|-----------|-------|
| 9.1         | test_project_load_success | unit |
| 9.3         | test_project_load_success | unit |
| 9.6         | test_project_load_no_files_error | unit |
| 9.8         | test_project_load_no_files_error | unit |
*/

// Deferred Requirements: none
