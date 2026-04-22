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

        let compose_files = if !config.files.is_empty() {
            config.files.clone()
        } else {
            Self::resolve_compose_files(&project_dir)?
        };

        let project_name = if let Some(name) = &config.project_name {
            name.clone()
        } else {
            Self::resolve_project_name(&project_dir)?
        };

        let env = yaml::load_env(&project_dir, &config.env_files);
        let spec = yaml::parse_and_merge_files(&compose_files, &env)?;

        Ok(Self {
            spec,
            project_name,
            project_dir,
            compose_files,
        })
    }

    fn resolve_compose_files(project_dir: &Path) -> Result<Vec<PathBuf>> {
        if let Ok(files_env) = std::env::var("COMPOSE_FILE") {
            let files: Vec<PathBuf> = files_env
                .split(':')
                .map(|s| project_dir.join(s))
                .collect();
            return Ok(files);
        }

        let candidates = ["compose.yaml", "compose.yml", "docker-compose.yaml", "docker-compose.yml"];
        for c in candidates {
            let path = project_dir.join(c);
            if path.exists() {
                return Ok(vec![path]);
            }
        }

        Err(ComposeError::FileNotFound {
            path: "compose.yaml or docker-compose.yml".into(),
        })
    }

    fn resolve_project_name(project_dir: &Path) -> Result<String> {
        if let Ok(name) = std::env::var("COMPOSE_PROJECT_NAME") {
            return Ok(name);
        }

        project_dir
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_lowercase().replace(|c: char| !c.is_alphanumeric(), "-"))
            .ok_or_else(|| ComposeError::ValidationError {
                message: "Could not derive project name from directory".into(),
            })
    }
}
