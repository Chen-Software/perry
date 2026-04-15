use std::path::PathBuf;

pub struct Config {
    pub files: Vec<PathBuf>,
    pub project_name: Option<String>,
    pub env_files: Vec<PathBuf>,
}

impl Config {
    pub fn from_env() -> Self {
        let mut files = Vec::new();
        if let Ok(f) = std::env::var("COMPOSE_FILE") {
            for file in f.split(':') {
                files.push(PathBuf::from(file));
            }
        } else {
            if std::path::Path::new("compose.yaml").exists() {
                files.push(PathBuf::from("compose.yaml"));
            } else if std::path::Path::new("docker-compose.yml").exists() {
                files.push(PathBuf::from("docker-compose.yml"));
            }
        }

        let project_name = std::env::var("COMPOSE_PROJECT_NAME").ok();

        Self {
            files,
            project_name,
            env_files: Vec::new(),
        }
    }
}
