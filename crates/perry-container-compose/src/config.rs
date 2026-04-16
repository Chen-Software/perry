use std::path::{Path, PathBuf};

pub struct ProjectConfig {
    pub files: Vec<PathBuf>,
    pub project_name: Option<String>,
    pub env_files: Vec<PathBuf>,
    pub project_dir: PathBuf,
}

impl ProjectConfig {
    pub fn new(files: Vec<PathBuf>, project_name: Option<String>, env_files: Vec<PathBuf>, project_dir: PathBuf) -> Self {
        Self { files, project_name, env_files, project_dir }
    }
}

pub fn resolve_project_name(name_flag: Option<String>, project_dir: &Path) -> String {
    if let Some(name) = name_flag {
        return name;
    }
    if let Ok(name) = std::env::var("COMPOSE_PROJECT_NAME") {
        return name;
    }
    project_dir.file_name().and_then(|n| n.to_str()).unwrap_or("perry").to_string()
}

pub fn resolve_compose_files(files_flag: Vec<PathBuf>, project_dir: &Path) -> Vec<PathBuf> {
    if !files_flag.is_empty() {
        return files_flag;
    }
    if let Ok(files) = std::env::var("COMPOSE_FILE") {
        return files.split(':').map(PathBuf::from).collect();
    }
    let candidates = ["compose.yaml", "compose.yml", "docker-compose.yaml", "docker-compose.yml"];
    for c in candidates {
        let p = project_dir.join(c);
        if p.exists() {
            return vec![p];
        }
    }
    vec![]
}
