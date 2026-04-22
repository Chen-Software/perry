use crate::error::Result;
use crate::config::ProjectConfig;
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
        let compose_files = config.resolve_compose_files();

        // Use CWD as project dir if no files, or parent of first file
        let project_dir = if let Some(first_file) = compose_files.first() {
            first_file.parent()
                .map(|p| if p.as_os_str().is_empty() { PathBuf::from(".") } else { p.to_path_buf() })
                .unwrap_or_else(|| PathBuf::from("."))
        } else {
            PathBuf::from(".")
        };

        let project_name = config.resolve_project_name(&project_dir);
        let env = load_env(&project_dir, &config.env_files);
        let spec = parse_and_merge_files(&compose_files, &env)?;

        Ok(Self {
            spec,
            project_name,
            project_dir,
            compose_files,
        })
    }
}
