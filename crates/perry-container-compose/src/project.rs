use std::path::PathBuf;
use crate::error::ComposeError;

pub struct ProjectConfig {
    pub files: Vec<PathBuf>,
    pub project_name: Option<String>,
}

pub struct ComposeProject {
    pub config: ProjectConfig,
}

impl ComposeProject {
    pub fn new(config: ProjectConfig) -> Self {
        Self { config }
    }

    pub fn discover() -> Result<ProjectConfig, ComposeError> {
        let mut files = Vec::new();
        for f in &["compose.yaml", "compose.yml", "docker-compose.yaml", "docker-compose.yml"] {
            if std::path::Path::new(f).exists() {
                files.push(PathBuf::from(f));
                break;
            }
        }

        if files.is_empty() {
            return Err(ComposeError::NotFound("No compose file found".to_string()));
        }

        Ok(ProjectConfig {
            files,
            project_name: None,
        })
    }
}
