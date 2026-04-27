use std::path::PathBuf;
use crate::types::ComposeSpec;
use crate::error::ComposeError;
use crate::yaml;

pub struct ComposeProject {
    pub name: String,
    pub working_dir: PathBuf,
    pub spec: ComposeSpec,
}

impl ComposeProject {
    pub fn load(files: Vec<PathBuf>, name: Option<String>) -> Result<Self, ComposeError> {
        let mut merged_spec: Option<ComposeSpec> = None;

        for file in files {
            let spec = yaml::parse_file(&file)?;
            if let Some(mut existing) = merged_spec {
                existing.services.extend(spec.services);
                // Simple merge for now
                merged_spec = Some(existing);
            } else {
                merged_spec = Some(spec);
            }
        }

        let spec = merged_spec.ok_or_else(|| ComposeError::InvalidConfig("No compose files provided".into()))?;
        let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let name = name.unwrap_or_else(|| {
            working_dir.file_name().and_then(|n| n.to_str()).unwrap_or("default").to_string()
        });

        Ok(Self { name, working_dir, spec })
    }
}
