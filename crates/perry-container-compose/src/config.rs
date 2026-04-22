use std::path::{Path, PathBuf};

pub struct ProjectConfig {
    pub compose_files: Vec<PathBuf>,
    pub project_name: String,
    pub project_dir: PathBuf,
    pub env_files: Vec<PathBuf>,
}

impl ProjectConfig {
    pub fn resolve(files: Vec<PathBuf>, project_name: Option<String>, env_files: Vec<PathBuf>) -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

        let compose_files = if files.is_empty() {
            let candidates = ["compose.yaml", "compose.yml", "docker-compose.yaml", "docker-compose.yml"];
            let mut found = Vec::new();
            for c in candidates {
                let p = cwd.join(c);
                if p.exists() {
                    found.push(p);
                    break;
                }
            }
            if found.is_empty() {
                found.push(cwd.join("compose.yaml"));
            }
            found
        } else {
            files
        };

        let project_dir = compose_files.first()
            .and_then(|f| f.parent())
            .map(|p| p.to_path_buf())
            .unwrap_or(cwd.clone());

        let project_name = project_name
            .or_else(|| std::env::var("COMPOSE_PROJECT_NAME").ok())
            .unwrap_or_else(|| {
                project_dir.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("perry-stack")
                    .to_string()
            });

        Self {
            compose_files,
            project_name,
            project_dir,
            env_files,
        }
    }
}
