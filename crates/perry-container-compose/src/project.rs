use crate::config::{self, ProjectConfig};
use crate::error::Result;
use crate::types::ComposeSpec;
use crate::yaml;
use std::path::PathBuf;

pub struct ComposeProject {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub project_dir: PathBuf,
    pub compose_files: Vec<PathBuf>,
}

impl ComposeProject {
    pub fn load(config: &ProjectConfig) -> Result<Self> {
        let project_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let env = yaml::load_env(&project_dir, &config.env_files);

        let compose_files = config::resolve_compose_files(&config.files, &env);
        let project_name = config::resolve_project_name(config.project_name.as_deref(), &project_dir, &env);

        let spec = yaml::parse_and_merge_files(&compose_files, &env)?;

        Ok(Self {
            spec,
            project_name,
            project_dir,
            compose_files,
        })
    }
}
