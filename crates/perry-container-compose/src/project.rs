//! Compose project management and file discovery.

use crate::config;
use crate::error::Result;
use crate::types::ComposeSpec;
use crate::yaml;
use std::path::{Path, PathBuf};

pub struct ComposeProject {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub project_dir: PathBuf,
    pub compose_files: Vec<PathBuf>,
}

impl ComposeProject {
    pub fn load(
        name_flag: Option<String>,
        file_flags: Vec<String>,
        env_file_flags: Vec<String>,
        project_dir: &Path,
    ) -> Result<Self> {
        let project_name = config::resolve_project_name(name_flag, project_dir);
        let compose_files = config::resolve_compose_files(file_flags, project_dir);
        let env_files: Vec<PathBuf> = env_file_flags.into_iter().map(PathBuf::from).collect();

        let env = yaml::load_env(project_dir, &env_files);
        let spec = yaml::parse_and_merge_files(&compose_files, &env)?;

        Ok(Self {
            spec,
            project_name,
            project_dir: project_dir.to_path_buf(),
            compose_files,
        })
    }
}
