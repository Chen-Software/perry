use std::path::PathBuf;

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

pub fn resolve_project_name(explicit_name: Option<&str>) -> crate::error::Result<String> {
    if let Some(name) = explicit_name {
        return Ok(name.to_string());
    }
    if let Ok(name) = std::env::var("COMPOSE_PROJECT_NAME") {
        return Ok(name);
    }
    let cwd = std::env::current_dir()?;
    let name = cwd.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("default")
        .to_string();
    Ok(name)
}

pub fn resolve_compose_files(explicit_files: &[PathBuf]) -> crate::error::Result<Vec<PathBuf>> {
    if !explicit_files.is_empty() {
        return Ok(explicit_files.to_vec());
    }
    if let Ok(files_env) = std::env::var("COMPOSE_FILE") {
        return Ok(files_env.split(':').map(PathBuf::from).collect());
    }
    let candidates = ["compose.yaml", "compose.yml", "docker-compose.yaml", "docker-compose.yml"];
    for c in candidates {
        let path = PathBuf::from(c);
        if path.exists() {
            return Ok(vec![path]);
        }
    }
    Ok(Vec::new())
}
