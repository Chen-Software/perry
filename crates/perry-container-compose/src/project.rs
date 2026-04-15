use crate::error::{ComposeError, Result};
use crate::types::ComposeSpec;
use crate::yaml::{load_env, parse_compose_yaml};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ComposeProject {
    pub name: String,
    pub spec: ComposeSpec,
    pub env: HashMap<String, String>,
}

impl ComposeProject {
    pub fn new(
        files: &[PathBuf],
        project_name: Option<String>,
        extra_env_files: &[PathBuf],
    ) -> Result<Self> {
        let mut final_spec = ComposeSpec::default();
        let mut base_env = HashMap::new();

        if files.is_empty() {
            return Err(ComposeError::ValidationError {
                message: "No compose files specified".to_string(),
            });
        }

        let project_dir = files[0].parent().unwrap_or(Path::new("."));
        base_env = load_env(project_dir, extra_env_files);

        for f in files {
            let content = std::fs::read_to_string(f).map_err(|e| ComposeError::IoError(e))?;
            let spec = parse_compose_yaml(&content, &base_env)?;
            final_spec.merge(spec);
        }

        let name = project_name
            .or_else(|| final_spec.name.clone())
            .or_else(|| {
                project_dir
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| "default".to_string());

        Ok(Self {
            name,
            spec: final_spec,
            env: base_env,
        })
    }
}
