use std::path::PathBuf;
use crate::error::Result;
use crate::types::ComposeSpec;
use crate::config::{ProjectConfig, resolve_project_name, resolve_compose_files};
use crate::yaml::{load_env, parse_and_merge_files};

pub struct ComposeProject {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub project_dir: PathBuf,
    pub compose_files: Vec<PathBuf>,
}

impl ComposeProject {
    pub fn load(config: ProjectConfig) -> Result<Self> {
        let compose_files = resolve_compose_files(config.files.iter().map(|p| p.to_string_lossy().into_owned()).collect());
        let project_name = resolve_project_name(&config);
        let project_dir = compose_files.first()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

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
