use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct ProjectConfig {
    pub files: Vec<PathBuf>,
    pub project_name: Option<String>,
    pub env_files: Vec<PathBuf>,
}

impl ProjectConfig {
    pub fn new(files: Vec<PathBuf>, project_name: Option<String>, env_files: Vec<PathBuf>) -> Self {
        Self {
            files,
            project_name,
            env_files,
        }
    }
}

pub fn resolve_project_name(
    explicit_name: Option<&str>,
    project_dir: &Path,
    env: &HashMap<String, String>,
) -> String {
    if let Some(name) = explicit_name {
        return name.to_string();
    }

    if let Some(name) = env.get("COMPOSE_PROJECT_NAME") {
        return name.to_string();
    }

    project_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("perry-stack")
        .to_string()
}

pub fn resolve_compose_files(explicit_files: &[PathBuf], env: &HashMap<String, String>) -> Vec<PathBuf> {
    if !explicit_files.is_empty() {
        return explicit_files.to_vec();
    }

    if let Some(files_str) = env.get("COMPOSE_FILE") {
        return files_str
            .split(':')
            .map(PathBuf::from)
            .collect();
    }

    let candidates = ["compose.yaml", "compose.yml", "docker-compose.yaml", "docker-compose.yml"];
    for c in candidates {
        let p = PathBuf::from(c);
        if p.exists() {
            return vec![p];
        }
    }

    vec![]
}
