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

    /// Resolve the project name following precedence:
    /// 1. CLI flag (-p)
    /// 2. COMPOSE_PROJECT_NAME environment variable
    /// 3. Parent directory name of the first compose file (or CWD)
    pub fn resolve_project_name(&self, project_dir: &Path) -> String {
        if let Some(name) = &self.project_name {
            return name.clone();
        }

        if let Ok(name) = std::env::var("COMPOSE_PROJECT_NAME") {
            return name;
        }

        project_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("default")
            .to_string()
    }

    /// Resolve compose files following precedence:
    /// 1. CLI flags (-f)
    /// 2. COMPOSE_FILE environment variable (colon-separated)
    /// 3. compose.yaml or docker-compose.yml in CWD
    pub fn resolve_compose_files(&self) -> Vec<PathBuf> {
        if !self.files.is_empty() {
            return self.files.clone();
        }

        if let Ok(files_env) = std::env::var("COMPOSE_FILE") {
            return files_env
                .split(':')
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
                .collect();
        }

        let candidates = ["compose.yaml", "compose.yml", "docker-compose.yaml", "docker-compose.yml"];
        for candidate in candidates {
            let path = PathBuf::from(candidate);
            if path.exists() {
                return vec![path];
            }
        }

        Vec::new()
    }
}
