use std::path::PathBuf;

pub struct ProjectConfig {
    pub compose_files: Vec<PathBuf>,
    pub project_name: String,
    pub project_dir: PathBuf,
    pub env_files: Vec<PathBuf>,
}

impl ProjectConfig {
    pub fn resolve(
        files: Vec<PathBuf>,
        name: Option<String>,
        env_files: Vec<PathBuf>,
    ) -> Self {
        let mut resolved_files = files;
        if resolved_files.is_empty() {
            if let Ok(val) = std::env::var("COMPOSE_FILE") {
                resolved_files = val.split(':').map(PathBuf::from).collect();
            } else {
                let yml = PathBuf::from("compose.yaml");
                if yml.exists() {
                    resolved_files.push(yml);
                } else {
                    let dyml = PathBuf::from("docker-compose.yml");
                    if dyml.exists() {
                        resolved_files.push(dyml);
                    }
                }
            }
        }

        let project_dir = resolved_files.first()
            .and_then(|f| f.parent())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

        let project_name = name
            .or_else(|| std::env::var("COMPOSE_PROJECT_NAME").ok())
            .unwrap_or_else(|| {
                project_dir.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("perry-stack")
                    .to_string()
            });

        Self {
            compose_files: resolved_files,
            project_name,
            project_dir,
            env_files,
        }
    }
}
