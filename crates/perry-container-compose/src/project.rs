use crate::config::ProjectConfig;
use crate::types::ComposeSpec;
use crate::yaml;
use std::path::PathBuf;

pub struct ComposeProject {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub project_dir: PathBuf,
    pub compose_files: Vec<PathBuf>,
}

impl ComposeProject {
    pub fn load(config: &ProjectConfig) -> crate::error::Result<Self> {
        let project_dir = config.files.first()
            .and_then(|f| f.parent())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        let env = yaml::load_env(&project_dir, &config.env_files);
        let spec = yaml::parse_and_merge_files(&config.files, &env)?;

        Ok(Self {
            spec,
            project_name: config.project_name.clone().unwrap_or_else(|| {
                project_dir.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("perry-project")
                    .to_string()
            }),
            project_dir,
            compose_files: config.files.clone(),
        })
    }

    pub fn load_from_files(files: &[PathBuf], project_name: Option<String>, env_files: &[PathBuf]) -> crate::error::Result<Self> {
        let config = ProjectConfig::new(files.to_vec(), project_name, env_files.to_vec());
        Self::load(&config)
    }
}
