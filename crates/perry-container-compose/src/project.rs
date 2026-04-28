use crate::error::Result;
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
        let compose_files = crate::config::resolve_compose_files(&config.compose_files)?;
        let project_dir = compose_files[0]
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .to_path_buf();

        let project_name = crate::config::resolve_project_name(
            config.project_name.as_deref(),
            &project_dir,
        );

        // TODO: Load .env files, interpolate YAML, parse and merge
        // For now, return default spec
        Ok(Self {
            spec: ComposeSpec::default(),
            project_name,
            project_dir,
            compose_files,
        })
    }
}
