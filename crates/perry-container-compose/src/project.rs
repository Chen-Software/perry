use crate::error::{ComposeError, Result};
use crate::config::{ProjectConfig, resolve_project_name, resolve_compose_files};
use crate::types::ComposeSpec;
use crate::yaml::{load_env, parse_and_merge_files};
use std::path::PathBuf;
use std::env;

pub struct ComposeProject {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub project_dir: PathBuf,
    pub compose_files: Vec<PathBuf>,
}

impl ComposeProject {
    pub fn load(config: &ProjectConfig) -> Result<Self> {
        let current_dir = env::current_dir().map_err(ComposeError::IoError)?;
        let project_dir = current_dir.clone();

        let compose_files = resolve_compose_files(config.files.clone());
        if compose_files.is_empty() {
             return Err(ComposeError::ValidationError { message: "No compose files found".into() });
        }

        let env_map = load_env(&project_dir, &config.env_files);
        let mut spec = parse_and_merge_files(&compose_files, &env_map)?;

        let project_name = resolve_project_name(config.project_name.clone(), &project_dir);
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
}
