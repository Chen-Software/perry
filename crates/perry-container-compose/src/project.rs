//! `ComposeProject` — project loading and file discovery.

use crate::config::{self, ProjectConfig};
use crate::error::Result;
use crate::types::ComposeSpec;
use crate::yaml;
use std::path::{Path, PathBuf};

/// A loaded and resolved compose project.
pub struct ComposeProject {
    /// Project name
    pub project_name: String,
    /// Working directory
    pub project_dir: PathBuf,
    /// Compose file paths
    pub compose_files: Vec<PathBuf>,
    /// Merged and interpolated compose spec
    pub spec: ComposeSpec,
    /// Resolved environment variables
    pub env: std::collections::HashMap<String, String>,
}

impl ComposeProject {
    /// Convenience: load from raw file paths, project name, and env files.
    pub fn load_from_files(
        files: &[PathBuf],
        project_name: Option<&str>,
        env_files: &[PathBuf],
    ) -> Result<Self> {
        let config = ProjectConfig::new(
            files.to_vec(),
            project_name.map(String::from),
            env_files.to_vec(),
        );
        Self::load(&config)
    }

    /// Load a project from configuration.
    pub fn load(config: &ProjectConfig) -> Result<Self> {
        // Resolve compose file paths
        let files = if config.compose_files.is_empty() {
            config::resolve_compose_files(&[])? // Use default lookup
        } else {
            config.compose_files.clone()
        };

        let working_dir = files[0]
            .parent()
            .unwrap_or(Path::new("."))
            .to_path_buf();

        // Load environment
        let env = yaml::load_env(&working_dir, &config.env_files);

        // Parse and merge compose files
        let spec = yaml::parse_and_merge_files(&files, &env)?;

        // Determine project name
        let name = config::resolve_project_name(
            config.project_name.as_deref(),
            &working_dir,
        );

        Ok(ComposeProject {
            project_name: name,
            project_dir: working_dir,
            compose_files: files,
            spec,
            env,
        })
    }
}
