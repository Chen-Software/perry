use crate::error::Result;
use crate::types::ComposeSpec;
use std::path::PathBuf;

pub struct ComposeProject {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub project_dir: PathBuf,
    pub compose_files: Vec<PathBuf>,
}

impl ComposeProject {
    pub fn load(
        files: &[PathBuf],
        project_name: Option<String>,
        env_files: &[PathBuf],
    ) -> Result<Self> {
        let project_dir = if let Some(first) = files.first() {
            first.parent().unwrap_or_else(|| std::path::Path::new(".")).to_path_buf()
        } else {
            std::env::current_dir().unwrap_or_else(|_| ".".into())
        };

        let env = crate::yaml::load_env(&project_dir, env_files);
        let spec = crate::yaml::parse_and_merge_files(files, &env)?;

        let name = project_name.unwrap_or_else(|| {
            project_dir
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "default".into())
        });

        Ok(Self {
            spec,
            project_name: name,
            project_dir,
            compose_files: files.to_vec(),
        })
    }
}
