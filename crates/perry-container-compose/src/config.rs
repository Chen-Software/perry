use std::path::PathBuf;

pub struct ProjectConfig {
    pub files: Vec<PathBuf>,
    pub project_name: Option<String>,
    pub env_files: Vec<PathBuf>,
}

pub fn resolve_project_name(config: &ProjectConfig) -> String {
    if let Some(name) = &config.project_name {
        return name.clone();
    }
    if let Ok(name) = std::env::var("COMPOSE_PROJECT_NAME") {
        return name;
    }
    if let Some(first_file) = config.files.first() {
        if let Some(parent) = first_file.parent() {
            if let Some(name) = parent.file_name().and_then(|n| n.to_str()) {
                return name.to_string();
            }
        }
    }
    "default".to_string()
}

pub fn resolve_compose_files(files: Vec<String>) -> Vec<PathBuf> {
    if !files.is_empty() {
        return files.into_iter().map(PathBuf::from).collect();
    }
    if let Ok(val) = std::env::var("COMPOSE_FILE") {
        return val.split(':').map(PathBuf::from).collect();
    }
    let candidates = ["compose.yaml", "docker-compose.yml"];
    for c in candidates {
        let path = PathBuf::from(c);
        if path.exists() {
            return vec![path];
        }
    }
    Vec::new()
}
