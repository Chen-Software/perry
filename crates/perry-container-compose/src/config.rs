use std::path::{Path, PathBuf};
use std::env;

#[derive(Debug, Clone, Default)]
pub struct ProjectConfig {
    pub project_name: Option<String>,
    pub compose_files: Vec<PathBuf>,
    pub env_files: Vec<PathBuf>,
    pub project_dir: Option<PathBuf>,
}

pub fn resolve_project_name(config_name: Option<&str>, project_dir: &Path) -> String {
    if let Some(name) = config_name {
        return name.to_string();
    }
    if let Ok(name) = env::var("COMPOSE_PROJECT_NAME") {
        return name;
    }
    project_dir.file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "perry-stack".to_string())
}

pub fn resolve_compose_files(config_files: &[PathBuf]) -> Vec<PathBuf> {
    if !config_files.is_empty() {
        return config_files.to_vec();
    }
    if let Ok(files) = env::var("COMPOSE_FILE") {
        return files.split(':').map(PathBuf::from).collect();
    }
    let candidates = ["compose.yaml", "compose.yml", "docker-compose.yaml", "docker-compose.yml"];
    for c in candidates {
        let p = Path::new(c);
        if p.exists() {
            return vec![p.to_path_buf()];
        }
    }
    Vec::new()
}
