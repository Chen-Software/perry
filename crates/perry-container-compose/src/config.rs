use std::path::PathBuf;

use std::env;

#[derive(Debug, Clone, Default)]
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

pub fn resolve_project_name(config_name: Option<&str>, project_dir: &std::path::Path) -> String {
    if let Some(name) = config_name {
        return name.to_string();
    }
    if let Ok(name) = env::var("COMPOSE_PROJECT_NAME") {
        return name;
    }
    project_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("perry-stack")
        .to_string()
}

pub fn resolve_compose_files(config_files: &[PathBuf]) -> Vec<PathBuf> {
    if !config_files.is_empty() {
        return config_files.to_vec();
    }
    if let Ok(files) = env::var("COMPOSE_FILE") {
        return files.split(':').map(PathBuf::from).collect();
    }
    let defaults = ["compose.yaml", "compose.yml", "docker-compose.yaml", "docker-compose.yml"];
    for d in defaults {
        let p = PathBuf::from(d);
        if p.exists() {
            return vec![p];
        }
    }
    Vec::new()
}
