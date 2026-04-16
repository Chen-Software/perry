use std::path::{Path, PathBuf};
use std::env;
use crate::error::{ComposeError, Result};

pub struct ProjectConfig {
    pub files: Vec<PathBuf>,
    pub project_name: Option<String>,
    pub env_files: Vec<PathBuf>,
}

impl ProjectConfig {
    pub fn new(files: Vec<PathBuf>, project_name: Option<String>, env_files: Vec<PathBuf>) -> Self {
        Self { files, project_name, env_files }
    }
}

pub fn resolve_project_name(explicit_name: Option<&str>, project_dir: &Path) -> String {
    if let Some(name) = explicit_name {
        return name.to_string();
    }
    if let Ok(name) = env::var("COMPOSE_PROJECT_NAME") {
        return name;
    }
    project_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("default")
        .to_string()
}

pub fn resolve_compose_files(explicit_files: &[PathBuf]) -> Result<Vec<PathBuf>> {
    if !explicit_files.is_empty() {
        return Ok(explicit_files.to_vec());
    }

    if let Ok(files_env) = env::var("COMPOSE_FILE") {
        let files: Vec<PathBuf> = files_env
            .split(':')
            .map(PathBuf::from)
            .collect();
        if !files.is_empty() {
            return Ok(files);
        }
    }

    let candidates = ["compose.yaml", "compose.yml", "docker-compose.yaml", "docker-compose.yml"];
    for c in candidates {
        let path = PathBuf::from(c);
        if path.exists() {
            return Ok(vec![path]);
        }
    }

    Err(ComposeError::FileNotFound { path: "compose.yaml".into() })
}
