use std::path::PathBuf;
use crate::error::{ComposeError, Result};
use crate::types::ComposeSpec;
use crate::config::ProjectConfig;
use crate::yaml::{load_env, parse_and_merge_files};

pub struct ComposeProject {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub project_dir: PathBuf,
    pub compose_files: Vec<PathBuf>,
}

impl ComposeProject {
    pub fn load(config: &ProjectConfig) -> Result<Self> {
        let project_dir = config.project_dir.clone();
        let env = load_env(&project_dir, &config.env_files);
        let spec = parse_and_merge_files(&config.compose_files, &env)?;

        Ok(Self {
            spec,
            project_name: config.project_name.clone(),
            project_dir,
            compose_files: config.compose_files.clone(),
        })
    }
}
