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
        use crate::yaml::{load_env, parse_and_merge_files};
        use crate::config::{resolve_project_name, resolve_compose_files};

        let project_name = resolve_project_name(config.project_name.as_deref())?;
        let compose_files = resolve_compose_files(&config.files)?;

        if compose_files.is_empty() {
            return Err(ComposeError::FileNotFound { path: "No compose files found".into() });
        }

        let project_dir = compose_files[0]
            .parent()
            .map(|p: &std::path::Path| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

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
