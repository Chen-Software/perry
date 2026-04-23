use crate::config::{resolve_compose_files, resolve_project_name, ProjectConfig};
use crate::error::Result;
use crate::types::ComposeSpec;
use crate::yaml::{load_env, parse_and_merge_files};
use std::path::{Path, PathBuf};

pub struct ComposeProject {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub project_dir: PathBuf,
    pub compose_files: Vec<PathBuf>,
}

impl ComposeProject {
    pub fn load(config: &ProjectConfig) -> Result<Self> {
        let compose_files = resolve_compose_files(&config.compose_files)?;

        // project_dir is the directory of the first compose file
        let project_dir = compose_files
            .first()
            .and_then(|f| f.parent())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        let project_name = resolve_project_name(config.project_name.as_deref(), &project_dir);

        let env = load_env(&project_dir, &config.env_files);
        let spec = parse_and_merge_files(&compose_files, &env)?;

        Ok(Self {
            spec,
            project_name,
            project_dir,
            compose_files,
        })
    }

    /// Load a project from a list of files. Shortcut for `load()`.
    pub fn load_from_files(
        files: &[PathBuf],
        project_name: Option<String>,
        env_files: &[PathBuf],
    ) -> Result<Self> {
        let config = ProjectConfig::new(files.to_vec(), project_name, env_files.to_vec());
        Self::load(&config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_temp_dir(suffix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("perry-project-test-{}", suffix));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn test_project_load_basic() {
        let dir = make_temp_dir("basic");
        let file = dir.join("compose.yaml");
        fs::write(
            &file,
            r#"
services:
  web:
    image: nginx
"#,
        )
        .unwrap();

        let config = ProjectConfig::new(vec![file.clone()], None, vec![]);
        let project = ComposeProject::load(&config).unwrap();

        assert_eq!(project.project_name, "perry-project-test-basic");
        assert_eq!(project.project_dir, dir);
        assert_eq!(project.compose_files, vec![file]);
        assert!(project.spec.services.contains_key("web"));
    }

    #[test]
    fn test_project_load_multi_file_merge() {
        let dir = make_temp_dir("merge");
        let f1 = dir.join("f1.yaml");
        let f2 = dir.join("f2.yaml");

        fs::write(&f1, "services:\n  s1:\n    image: i1\n  s2:\n    image: i1").unwrap();
        fs::write(&f2, "services:\n  s2:\n    image: i2").unwrap();

        let project = ComposeProject::load_from_files(&[f1, f2], Some("custom".into()), &[]).unwrap();

        assert_eq!(project.project_name, "custom");
        assert_eq!(project.spec.services["s1"].image.as_deref(), Some("i1"));
        assert_eq!(project.spec.services["s2"].image.as_deref(), Some("i2"));
    }

    #[test]
    fn test_project_load_with_env_interpolation() {
        let dir = make_temp_dir("env");
        let file = dir.join("compose.yaml");
        let env_file = dir.join(".env");

        fs::write(&file, "services:\n  web:\n    image: ${IMG}").unwrap();
        fs::write(&env_file, "IMG=redis:alpine").unwrap();

        let project = ComposeProject::load_from_files(&[file], None, &[]).unwrap();
        assert_eq!(project.spec.services["web"].image.as_deref(), Some("redis:alpine"));
    }
}
