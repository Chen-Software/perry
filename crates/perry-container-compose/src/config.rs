use std::path::PathBuf;

pub struct ProjectConfig {
    pub files: Vec<PathBuf>,
    pub project_name: Option<String>,
    pub env_files: Vec<PathBuf>,
}

pub fn resolve_project_name(project_name: Option<String>, project_dir: &std::path::Path) -> String {
    if let Some(name) = project_name {
        return name;
    }
    if let Ok(name) = std::env::var("COMPOSE_PROJECT_NAME") {
        return name;
    }
    project_dir.file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "perry-stack".to_string())
}

pub fn resolve_compose_files(files: Vec<PathBuf>) -> Vec<PathBuf> {
    if !files.is_empty() {
        return files;
    }
    if let Ok(val) = std::env::var("COMPOSE_FILE") {
        return val.split(':').map(PathBuf::from).collect();
    }

    let candidates = vec!["compose.yaml", "compose.yml", "docker-compose.yaml", "docker-compose.yml"];
    for c in candidates {
        let path = PathBuf::from(c);
        if path.exists() {
            return vec![path];
        }
    }

    Vec::new()
}
