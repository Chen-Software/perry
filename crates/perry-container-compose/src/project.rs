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
        let env = crate::yaml::load_env(&std::env::current_dir()?, &config.env_files);
        let project_name = crate::config::resolve_project_name(config.project_name.as_deref())?;
        let files = crate::config::resolve_compose_files(&config.files)?;

        let spec = crate::yaml::parse_and_merge_files(&files, &env)?;

        let project_dir = if let Some(first) = files.first() {
            first.parent().unwrap_or(std::path::Path::new(".")).to_path_buf()
        } else {
            std::env::current_dir()?
        };

        Ok(Self {
            spec,
            project_name,
            project_dir,
            compose_files: files,
        })
    }
}
