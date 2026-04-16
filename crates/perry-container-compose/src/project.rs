use crate::error::{ComposeError, Result};
use crate::types::ComposeSpec;
use crate::yaml::{load_env, parse_compose_yaml};
use std::path::PathBuf;

pub struct ComposeProject {
    pub name: String,
    pub working_dir: PathBuf,
    pub spec: ComposeSpec,
}

impl ComposeProject {
    pub fn new(
        name: Option<String>,
        files: &[PathBuf],
        env_files: &[PathBuf],
        working_dir: Option<PathBuf>,
    ) -> Result<Self> {
        let working_dir = working_dir.unwrap_or_else(|| std::env::current_dir().unwrap());
        let env = load_env(&working_dir, env_files);

        let mut merged_spec = ComposeSpec::default();

        let files = if files.is_empty() {
            vec![working_dir.join("docker-compose.yml"), working_dir.join("compose.yaml")]
        } else {
            files.to_vec()
        };

        let mut found = false;
        for f in files {
            if f.exists() {
                let content = std::fs::read_to_string(&f).map_err(ComposeError::IoError)?;
                let spec = parse_compose_yaml(&content, &env)?;
                merged_spec.merge(spec);
                found = true;
            }
        }

        if !found {
            return Err(ComposeError::FileNotFound { path: "No compose files found".into() });
        }

        let project_name = name.or_else(|| merged_spec.name.clone())
            .or_else(|| working_dir.file_name().and_then(|n| n.to_str().map(|s| s.to_string())))
            .unwrap_or_else(|| "default".to_string());

        Ok(ComposeProject {
            name: project_name,
            working_dir,
            spec: merged_spec,
        })
    }
}
