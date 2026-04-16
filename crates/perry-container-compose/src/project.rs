use crate::error::{ComposeError, Result};
use crate::config::ProjectConfig;
use crate::types::ComposeSpec;
use crate::yaml;
use std::path::{Path, PathBuf};

pub struct ComposeProject {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub project_dir: PathBuf,
    pub compose_files: Vec<PathBuf>,
}

impl ComposeProject {
    pub fn load(config: &ProjectConfig) -> Result<Self> {
        let project_dir = std::env::current_dir().map_err(ComposeError::IoError)?;

        // 1. Resolve compose files
        let compose_files = if config.files.is_empty() {
            Self::resolve_compose_files(&project_dir)?
        } else {
            config.files.clone()
        };

        if compose_files.is_empty() {
            return Err(ComposeError::FileNotFound {
                path: "compose.yaml or docker-compose.yml".to_string(),
            });
        }

        // 2. Load environment
        let env = yaml::load_env(&project_dir, &config.env_files);

        // 3. Parse and merge files
        let mut spec = yaml::parse_and_merge_files(&compose_files, &env)?;

        // 4. Resolve project name
        let project_name = if let Some(name) = &config.project_name {
            name.clone()
        } else if let Ok(name) = std::env::var("COMPOSE_PROJECT_NAME") {
            name
        } else if let Some(name) = &spec.name {
            name.clone()
        } else {
            project_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("default")
                .to_string()
        };

        // Ensure spec has the project name
        if spec.name.is_none() {
            spec.name = Some(project_name.clone());
        }

        Ok(Self {
            spec,
            project_name,
            project_dir,
            compose_files,
        })
    }

    fn resolve_compose_files(project_dir: &Path) -> Result<Vec<PathBuf>> {
        // Check COMPOSE_FILE env var first
        if let Ok(files_str) = std::env::var("COMPOSE_FILE") {
            let files: Vec<PathBuf> = files_str
                .split(':')
                .map(|s| project_dir.join(s))
                .collect();
            return Ok(files);
        }

        // Default candidates
        let candidates = ["compose.yaml", "compose.yml", "docker-compose.yaml", "docker-compose.yml"];
        for c in candidates {
            let path = project_dir.join(c);
            if path.exists() {
                return Ok(vec![path]);
            }
        }

        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProjectConfig;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_project_load_basic() {
        let dir = tempdir().unwrap();
        let compose_path = dir.path().join("compose.yaml");
        let mut file = File::create(&compose_path).unwrap();
        writeln!(
            file,
            r#"
name: my-project
services:
  web:
    image: nginx
"#
        )
        .unwrap();

        // Change current directory to temp dir for resolution
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        let config = ProjectConfig::new(vec![], None, vec![]);
        let project = ComposeProject::load(&config).unwrap();

        assert_eq!(project.project_name, "my-project");
        assert_eq!(project.compose_files, vec![compose_path]);
        assert!(project.spec.services.contains_key("web"));

        std::env::set_current_dir(original_dir).unwrap();
    }
}
