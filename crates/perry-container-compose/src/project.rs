use crate::error::{ComposeError, Result};
use crate::config::ProjectConfig;
use crate::types::ComposeSpec;
use std::path::PathBuf;

pub struct ComposeProject {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub project_dir: PathBuf,
    pub compose_files: Vec<PathBuf>,
}

impl ComposeProject {
    pub fn load(config: &ProjectConfig) -> Result<Self> {
        let project_dir = if let Some(first) = config.compose_files.first() {
            first.parent().unwrap_or(std::path::Path::new(".")).to_path_buf()
        } else {
            std::env::current_dir().map_err(ComposeError::IoError)?
        };

        let project_name = crate::config::resolve_project_name(config.project_name.as_deref(), &project_dir);
        let env = crate::yaml::load_env(&project_dir, &config.env_files);
        let spec = crate::yaml::parse_and_merge_files(&config.compose_files, &env)?;

        Ok(Self {
            spec,
            project_name,
            project_dir,
            compose_files: config.compose_files.clone(),
        })
    }

    pub fn load_from_files(files: &[PathBuf], project_name: Option<String>, env_files: &[PathBuf]) -> Result<Self> {
        let config = ProjectConfig::new(files.to_vec(), project_name, env_files.to_vec());
        Self::load(&config)
    }
}
