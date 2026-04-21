use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

pub fn resolve_project_name(name: Option<&str>) -> crate::error::Result<String> {
    if let Some(n) = name {
        return Ok(n.to_string());
    }
    if let Ok(n) = std::env::var("COMPOSE_PROJECT_NAME") {
        return Ok(n);
    }
    std::env::current_dir()
        .map_err(|e| crate::error::ComposeError::IoError(e))?
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .ok_or_else(|| crate::error::ComposeError::ValidationError {
            message: "Could not derive project name from current directory".into(),
        })
}

pub fn resolve_compose_files(files: &[PathBuf]) -> crate::error::Result<Vec<PathBuf>> {
    if !files.is_empty() {
        return Ok(files.to_vec());
    }
    if let Ok(env_files) = std::env::var("COMPOSE_FILE") {
        return Ok(env_files.split(':').map(PathBuf::from).collect());
    }
    for name in &["compose.yaml", "compose.yml", "docker-compose.yaml", "docker-compose.yml"] {
        let path = PathBuf::from(name);
        if path.exists() {
            return Ok(vec![path]);
        }
    }
    Ok(vec![])
}
