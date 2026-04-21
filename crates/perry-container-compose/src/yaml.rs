use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;
use regex::Regex;
use crate::error::{ComposeError, Result};
use crate::types::ComposeSpec;

pub fn interpolate_yaml(yaml: &str, env: &HashMap<String, String>) -> String {
    // 1. Handle escaped $$
    let yaml = yaml.replace("$$", "\u{0000}");

    let re = Regex::new(r"\$\{(?P<var>[A-Z0-9_]+)(?::-(?P<default>[^}]*))?(?:\+(?P<value>[^}]*))?\}").unwrap();
    let result = re.replace_all(&yaml, |caps: &regex::Captures| {
        let var = &caps["var"];
        let val = env.get(var).map(|s| s.as_str()).unwrap_or("");

        if !val.is_empty() {
            if let Some(_plus) = caps.name("value") {
                caps["value"].to_string()
            } else {
                val.to_string()
            }
        } else {
            // Variable is missing or empty
            if let Some(default) = caps.name("default") {
                default.as_str().to_string()
            } else if caps.name("value").is_some() {
                "".to_string()
            } else {
                val.to_string()
            }
        }
    }).to_string();

    // Restore escaped $
    result.replace("\u{0000}", "$")
}

pub fn load_env(project_dir: &Path, extra_env_files: &[PathBuf]) -> HashMap<String, String> {
    let mut env = HashMap::new();

    // Process env takes precedence
    for (k, v) in std::env::vars() {
        env.insert(k, v);
    }

    // .env file in project dir
    let dot_env = project_dir.join(".env");
    if dot_env.exists() {
        if let Ok(iter) = dotenvy::from_path_iter(&dot_env) {
            for item in iter {
                if let Ok((k, v)) = item {
                    env.entry(k).or_insert(v);
                }
            }
        }
    }

    // Explicit env files
    for path in extra_env_files {
        if let Ok(iter) = dotenvy::from_path_iter(path) {
            for item in iter {
                if let Ok((k, v)) = item {
                    env.entry(k).or_insert(v);
                }
            }
        }
    }

    env
}

pub fn parse_compose_yaml(yaml: &str, env: &HashMap<String, String>) -> Result<ComposeSpec> {
    let interpolated = interpolate_yaml(yaml, env);
    let spec: ComposeSpec = serde_yaml::from_str(&interpolated)?;
    Ok(spec)
}

pub fn parse_and_merge_files(files: &[PathBuf], env: &HashMap<String, String>) -> Result<ComposeSpec> {
    if files.is_empty() {
        return Err(ComposeError::validation("No compose files provided".into()));
    }

    let mut merged: Option<ComposeSpec> = None;
    for file in files {
        let content = fs::read_to_string(file).map_err(|_| ComposeError::FileNotFound { path: file.display().to_string() })?;
        let spec = parse_compose_yaml(&content, env)?;
        if let Some(ref mut m) = merged {
            m.merge(spec);
        } else {
            merged = Some(spec);
        }
    }

    Ok(merged.unwrap())
}
