use std::path::PathBuf;
use crate::error::Result;
use crate::types::ComposeSpec;
use crate::config::{ProjectConfig, resolve_project_name, resolve_compose_files};
use crate::yaml::{load_env, parse_and_merge_files};

pub struct ComposeProject {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub project_dir: PathBuf,
}

impl ComposeProject {
    pub fn load(config: ProjectConfig) -> Result<Self> {
        let project_dir = config.project_dir.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));
        let project_name = resolve_project_name(config.project_name.as_deref(), &project_dir);
        let files = resolve_compose_files(&config.compose_files);

        let env = load_env(&project_dir, &config.env_files);
        let spec = parse_and_merge_files(&files, &env)?;

        Ok(Self {
            spec,
            project_name,
            project_dir,
        })
    }
}
