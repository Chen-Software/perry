use std::path::{Path, PathBuf};
use std::env;

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

pub fn resolve_project_name(
    explicit_name: Option<String>,
    project_dir: &Path,
) -> String {
    // 1. Explicit name via -p / --project-name
    if let Some(name) = explicit_name {
        return name;
    }

    // 2. COMPOSE_PROJECT_NAME env var
    if let Ok(name) = env::var("COMPOSE_PROJECT_NAME") {
        if !name.is_empty() {
            return name;
        }
    }

    // 3. Current directory name
    let dir_name = project_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("default");

    // Normalize: lowercase, alphanumeric + dashes
    dir_name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect()
}

pub fn resolve_compose_files(
    explicit_files: Vec<PathBuf>,
) -> Vec<PathBuf> {
    // 1. Explicit files via -f / --file
    if !explicit_files.is_empty() {
        return explicit_files;
    }

    // 2. COMPOSE_FILE env var
    if let Ok(files_str) = env::var("COMPOSE_FILE") {
        let sep = if cfg!(windows) { ';' } else { ':' };
        let paths: Vec<PathBuf> = files_str
            .split(sep)
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .collect();
        if !paths.is_empty() {
            return paths;
        }
    }

    // 3. Default files
    let defaults = [
        "compose.yaml",
        "compose.yml",
        "docker-compose.yaml",
        "docker-compose.yml",
    ];

    for name in defaults {
        let path = PathBuf::from(name);
        if path.exists() {
            return vec![path];
        }
    }

    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_resolve_project_name() {
        let dir = tempdir().unwrap();
        let project_dir = dir.path().join("my-project");
        fs::create_dir(&project_dir).unwrap();

        // 1. Explicit
        assert_eq!(resolve_project_name(Some("foo".into()), &project_dir), "foo");

        // 2. Env var
        env::set_var("COMPOSE_PROJECT_NAME", "env-name");
        assert_eq!(resolve_project_name(None, &project_dir), "env-name");
        env::remove_var("COMPOSE_PROJECT_NAME");

        // 3. Dir name
        assert_eq!(resolve_project_name(None, &project_dir), "my-project");
    }

    #[test]
    fn test_resolve_compose_files() {
        let dir = tempdir().unwrap();
        let _ = env::set_current_dir(dir.path());

        // 1. Explicit
        let explicit = vec![PathBuf::from("custom.yaml")];
        assert_eq!(resolve_compose_files(explicit.clone()), explicit);

        // 2. Env var
        env::set_var("COMPOSE_FILE", "env1.yaml:env2.yaml");
        assert_eq!(resolve_compose_files(vec![]), vec![PathBuf::from("env1.yaml"), PathBuf::from("env2.yaml")]);
        env::remove_var("COMPOSE_FILE");

        // 3. Default
        fs::write(dir.path().join("compose.yaml"), "").unwrap();
        assert_eq!(resolve_compose_files(vec![]), vec![PathBuf::from("compose.yaml")]);
    }
}
