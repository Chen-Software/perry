use crate::types::ComposeSpec;
use crate::error::{ComposeError, Result};
use crate::yaml;
use crate::config::ComposeConfig;
use std::path::{Path, PathBuf};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ComposeProject {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub project_dir: PathBuf,
    pub compose_files: Vec<PathBuf>,
}

impl ComposeProject {
    pub fn load(config: &ComposeConfig) -> Result<Self> {
        let project_dir = if let Some(first_file) = config.files.first() {
            first_file.parent().unwrap_or(Path::new(".")).to_path_buf()
        } else {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        };

        let env = yaml::load_env(&project_dir, &config.env_files);
        let spec = yaml::parse_and_merge_files(&config.files, &env)?;

        let project_name = config.project_name.clone().or_else(|| {
            std::env::var("COMPOSE_PROJECT_NAME").ok()
        }).unwrap_or_else(|| {
            project_dir.file_name().and_then(|n| n.to_str()).unwrap_or("default").to_string()
        });

        Ok(Self {
            spec,
            project_name,
            project_dir,
            compose_files: config.files.clone(),
        })
    }
}
