//! Project configuration and environment variable resolution.

use std::path::PathBuf;

/// Project configuration for compose operations.
#[derive(Debug, Clone)]
pub struct ComposeConfig {
    pub project_name: String,
    pub compose_files: Vec<PathBuf>,
    pub env_files: Vec<PathBuf>,
}

/// Resolve project name from flag, env var, or directory name.
pub fn resolve_project_name(
    name_flag: Option<String>,
    project_dir: &std::path::Path,
) -> String {
    if let Some(name) = name_flag {
        return name;
    }
    if let Ok(name) = std::env::var("COMPOSE_PROJECT_NAME") {
        return name;
    }
    project_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("default")
        .to_string()
}

/// Resolve compose files from flag, env var, or default filenames.
pub fn resolve_compose_files(
    file_flags: Vec<String>,
    project_dir: &std::path::Path,
) -> Vec<PathBuf> {
    if !file_flags.is_empty() {
        return file_flags.into_iter().map(PathBuf::from).collect();
    }
    if let Ok(files) = std::env::var("COMPOSE_FILE") {
        return files.split(':').map(PathBuf::from).collect();
    }
    let defaults = ["compose.yaml", "compose.yml", "docker-compose.yaml", "docker-compose.yml"];
    for name in defaults {
        let path = project_dir.join(name);
        if path.exists() {
            return vec![path];
        }
    }
    Vec::new()
}
