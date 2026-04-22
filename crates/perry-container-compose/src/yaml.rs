use std::collections::HashMap;
use std::path::Path;
use crate::error::{ComposeError, Result};
use crate::types::ComposeSpec;
use regex::Regex;

pub fn interpolate_yaml(yaml: &str, env: &HashMap<String, String>) -> String {
    let re = Regex::new(r"\$\{([^}:]+)(?::(-|\+)([^}]*))?\}").unwrap();
    re.replace_all(yaml, |caps: &regex::Captures| {
        let var_name = &caps[1];
        let modifier = caps.get(2).map(|m| m.as_str());
        let modifier_val = caps.get(3).map(|m| m.as_str()).unwrap_or("");

        match env.get(var_name) {
            Some(v) if !v.is_empty() => {
                if modifier == Some("+") {
                    modifier_val.to_string()
                } else {
                    v.clone()
                }
            }
            _ => {
                if modifier == Some("-") {
                    modifier_val.to_string()
                } else if modifier == Some("+") {
                    "".to_string()
                } else {
                    // Default behavior for ${VAR} if not found: empty string or keep it?
                    // Typically compose keeps it or replaces with empty.
                    "".to_string()
                }
            }
        }
    }).to_string()
}

pub fn load_env(project_dir: &Path, extra_env_files: &[std::path::PathBuf]) -> HashMap<String, String> {
    let mut env: HashMap<String, String> = std::env::vars().collect();

    let default_env = project_dir.join(".env");
    if default_env.exists() {
        if let Ok(content) = std::fs::read_to_string(&default_env) {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') { continue; }
                if let Some((k, v)) = line.split_once('=') {
                    env.entry(k.trim().to_string()).or_insert(v.trim().to_string());
                }
            }
        }
    }

    for ef in extra_env_files {
        if let Ok(content) = std::fs::read_to_string(ef) {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') { continue; }
                if let Some((k, v)) = line.split_once('=') {
                    env.insert(k.trim().to_string(), v.trim().to_string());
                }
            }
        }
    }

    env
}

pub fn parse_compose_yaml(yaml: &str, env: &HashMap<String, String>) -> Result<ComposeSpec> {
    let interpolated = interpolate_yaml(yaml, env);
    ComposeSpec::parse_str(&interpolated)
}

pub fn parse_and_merge_files(files: &[std::path::PathBuf], env: &HashMap<String, String>) -> Result<ComposeSpec> {
    let mut root_spec = ComposeSpec::default();
    for file in files {
        if !file.exists() {
            return Err(ComposeError::FileNotFound { path: file.display().to_string() });
        }
        let content = std::fs::read_to_string(file).map_err(ComposeError::IoError)?;
        let spec = parse_compose_yaml(&content, env)?;
        if root_spec.services.is_empty() {
            root_spec = spec;
        } else {
            root_spec.merge(spec);
        }
    }
    Ok(root_spec)
}
pub fn parse_dotenv(content: &str) -> HashMap<String, String> {
    let mut env = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        if let Some((k, v)) = line.split_once('=') {
            env.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    env
}
