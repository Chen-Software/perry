use crate::error::{ComposeError, Result};
use crate::config::{ProjectConfig, resolve_project_name, resolve_compose_files};
use crate::types::ComposeSpec;
use crate::yaml;
use std::path::PathBuf;
use std::env;

pub struct ComposeProject {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub project_dir: PathBuf,
    pub compose_files: Vec<PathBuf>,
}

impl ComposeProject {
    pub fn load_from_files(
        files: &[PathBuf],
        project_name: Option<String>,
        env_files: &[PathBuf],
    ) -> Result<Self> {
        let project_dir = env::current_dir().map_err(ComposeError::IoError)?;
        let resolved_files = resolve_compose_files(files.to_vec());
        if resolved_files.is_empty() {
            return Err(ComposeError::FileNotFound {
                path: "compose.yaml (default)".to_string(),
            });
        }
        let resolved_name = resolve_project_name(project_name, &project_dir);
        let env_map = yaml::load_env(&project_dir, env_files);
        let spec = yaml::parse_and_merge_files(&resolved_files, &env_map)?;

        Ok(Self {
            spec,
            project_name: resolved_name,
            project_dir,
            compose_files: resolved_files,
        })
    }

    pub fn load(config: &ProjectConfig) -> Result<Self> {
        Self::load_from_files(&config.files, config.project_name.clone(), &config.env_files)
    }
}
