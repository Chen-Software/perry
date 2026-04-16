use std::path::{Path, PathBuf};

pub struct ProjectConfig {
    pub files: Vec<PathBuf>,
    pub project_name: Option<String>,
    pub env_files: Vec<PathBuf>,
}

impl ProjectConfig {
    pub fn new(files: Vec<PathBuf>, project_name: Option<String>, env_files: Vec<PathBuf>) -> Self {
        Self { files, project_name, env_files }
    }

    /// Resolve the project name from flags, environment, or directory name.
    pub fn resolve_project_name(&self, project_dir: &Path) -> String {
        if let Some(name) = &self.project_name {
            return name.clone();
        }

        if let Ok(name) = std::env::var("COMPOSE_PROJECT_NAME") {
            if !name.is_empty() {
                return name;
            }
        }

        project_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("default")
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect()
    }

    /// Resolve compose files from flags, environment, or defaults.
    pub fn resolve_compose_files(&self, project_dir: &Path) -> Vec<PathBuf> {
        if !self.files.is_empty() {
            return self.files.clone();
        }

        if let Ok(files_env) = std::env::var("COMPOSE_FILE") {
            let files: Vec<PathBuf> = files_env
                .split(':')
                .filter(|s| !s.is_empty())
                .map(|s| project_dir.join(s))
                .collect();
            if !files.is_empty() {
                return files;
            }
        }

        // Default files
        let candidates = ["compose.yaml", "compose.yml", "docker-compose.yaml", "docker-compose.yml"];
        for c in candidates {
            let p = project_dir.join(c);
            if p.exists() {
                return vec![p];
            }
        }

        Vec::new()
    }
}
