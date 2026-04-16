use crate::error::{ComposeError, Result};
use crate::config::{ProjectConfig, resolve_project_name, resolve_compose_files};
use crate::types::ComposeSpec;
use crate::yaml::load_env;
use std::path::PathBuf;

pub struct ComposeProject {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub project_dir: PathBuf,
    pub compose_files: Vec<PathBuf>,
}

impl ComposeProject {
    pub fn load(config: &ProjectConfig) -> Result<Self> {
        let project_name = resolve_project_name(config.project_name.clone(), &config.project_dir);
        let compose_files = resolve_compose_files(config.files.clone(), &config.project_dir);

        if compose_files.is_empty() {
            return Err(ComposeError::FileNotFound { path: "no compose file found".into() });
        }

        let env = load_env(&config.project_dir, &config.env_files);
        let spec = crate::yaml::parse_and_merge_files(&compose_files, &env)?;

        Ok(Self {
            spec,
            project_name,
            project_dir: config.project_dir.clone(),
            compose_files,
        })
    }
}
