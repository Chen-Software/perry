use std::path::{Path, PathBuf};
use crate::error::Result;
use crate::types::ComposeSpec;
use crate::yaml;
use crate::config::{ProjectConfig, resolve_project_name, resolve_compose_files};

pub struct ComposeProject {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub project_dir: PathBuf,
    pub compose_files: Vec<PathBuf>,
}

impl ComposeProject {
    pub fn load(config: ProjectConfig) -> Result<Self> {
        let project_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let project_name = resolve_project_name(config.project_name, &project_dir);
        let compose_files = resolve_compose_files(config.files);

        let env = yaml::load_env(&project_dir, &config.env_files);
        let spec = yaml::parse_and_merge_files(&compose_files, &env)?;

        Ok(Self {
            spec,
            project_name,
            project_dir,
            compose_files,
        })
    }
}
