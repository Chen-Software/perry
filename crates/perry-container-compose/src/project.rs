//! Project model — `ComposeProject` struct and file discovery.

use crate::config::{resolve_compose_files, resolve_project_name, ProjectConfig};
use crate::error::Result;
use crate::types::ComposeSpec;
use crate::yaml::{load_env, parse_and_merge_files};
use std::path::PathBuf;

pub struct ComposeProject {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub project_dir: PathBuf,
    pub compose_files: Vec<PathBuf>,
}

impl ComposeProject {
    pub fn load(config: &ProjectConfig) -> Result<Self> {
        let compose_files = resolve_compose_files(&config.compose_files)?;
        let project_dir = compose_files[0].parent().unwrap_or(&std::path::Path::new(".")).to_path_buf();
        let project_name = resolve_project_name(config.project_name.as_deref(), &project_dir);
        let env = load_env(&project_dir, &config.env_files);
        let spec = parse_and_merge_files(&compose_files, &env)?;
        Ok(ComposeProject { spec, project_name, project_dir, compose_files })
    }
}
