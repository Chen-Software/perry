use std::path::{Path, PathBuf};
use std::env;

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

pub fn resolve_project_name(explicit_name: Option<String>, project_dir: &Path) -> String {
    if let Some(name) = explicit_name {
        return name;
    }
    if let Ok(name) = env::var("COMPOSE_PROJECT_NAME") {
        return name;
    }
    project_dir
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("default")
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

pub fn resolve_compose_files(explicit_files: Vec<PathBuf>) -> Vec<PathBuf> {
    if !explicit_files.is_empty() {
        return explicit_files;
    }

    if let Ok(files_env) = env::var("COMPOSE_FILE") {
        let sep = if cfg!(windows) { ";" } else { ":" };
        return files_env.split(sep).map(PathBuf::from).collect();
    }

    let defaults = ["compose.yaml", "compose.yml", "docker-compose.yaml", "docker-compose.yml"];
    for &f in &defaults {
        let path = PathBuf::from(f);
        if path.exists() {
            return vec![path];
        }
    }

    Vec::new()
}
