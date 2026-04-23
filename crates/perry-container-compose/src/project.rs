use std::path::{Path, PathBuf};
use std::env;
use crate::types::*;
use crate::error::ComposeError;
use anyhow::{Result, anyhow};

pub struct ComposeProject;

impl ComposeProject {
    pub fn load(path_or_spec: &str) -> Result<ComposeSpec, ComposeError> {
        if path_or_spec.trim().starts_with('{') {
            return serde_json::from_str(path_or_spec).map_err(|e| ComposeError::JsonError(e.to_string()));
        }

        let base_path = Path::new(path_or_spec);
        let config_file = if base_path.is_dir() {
            Self::find_compose_file(base_path).ok_or_else(|| ComposeError::FileNotFound("No compose file found".to_string()))?
        } else {
            base_path.to_path_buf()
        };

        let content = std::fs::read_to_string(&config_file).map_err(ComposeError::IoError)?;
        let yaml_val: serde_yaml::Value = serde_yaml::from_str(&content).map_err(|e| ComposeError::ParseError(e.to_string()))?;

        let interpolated = Self::interpolate_value(yaml_val)?;
        let spec: ComposeSpec = serde_json::from_value(serde_json::to_value(interpolated).map_err(|e| ComposeError::JsonError(e.to_string()))?)
            .map_err(|e| ComposeError::JsonError(e.to_string()))?;

        Ok(spec)
    }

    fn find_compose_file(dir: &Path) -> Option<PathBuf> {
        let files = ["compose.yaml", "compose.yml", "docker-compose.yaml", "docker-compose.yml"];
        for f in files {
            let p = dir.join(f);
            if p.exists() { return Some(p); }
        }
        None
    }

    fn interpolate_value(val: serde_yaml::Value) -> Result<serde_yaml::Value, ComposeError> {
        match val {
            serde_yaml::Value::String(s) => {
                let interpolated = Self::interpolate_string(&s);
                Ok(serde_yaml::Value::String(interpolated))
            }
            serde_yaml::Value::Mapping(map) => {
                let mut new_map = serde_yaml::Mapping::new();
                for (k, v) in map {
                    new_map.insert(k, Self::interpolate_value(v)?);
                }
                Ok(serde_yaml::Value::Mapping(new_map))
            }
            serde_yaml::Value::Sequence(seq) => {
                let mut new_seq = Vec::new();
                for v in seq {
                    new_seq.push(Self::interpolate_value(v)?);
                }
                Ok(serde_yaml::Value::Sequence(new_seq))
            }
            _ => Ok(val),
        }
    }

    fn interpolate_string(s: &str) -> String {
        let mut result = String::new();
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '$' {
                if let Some('{') = chars.peek() {
                    chars.next();
                    let mut var = String::new();
                    while let Some(vc) = chars.next() {
                        if vc == '}' { break; }
                        var.push(vc);
                    }
                    if let Ok(val) = env::var(&var) { result.push_str(&val); }
                } else {
                    let mut var = String::new();
                    while let Some(&vc) = chars.peek() {
                        if vc.is_alphanumeric() || vc == '_' {
                            var.push(chars.next().unwrap());
                        } else { break; }
                    }
                    if let Ok(val) = env::var(&var) { result.push_str(&val); }
                    else { result.push('$'); result.push_str(&var); }
                }
            } else {
                result.push(c);
            }
        }
        result
    }
}
