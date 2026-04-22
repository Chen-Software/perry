use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use crate::error::{ComposeError, Result};
use crate::types::ComposeSpec;

/// Interpolate ${VAR}, ${VAR:-default}, ${VAR:+value} in a YAML string.
pub fn interpolate(yaml: &str, env: &HashMap<String, String>) -> String {
    let re = Regex::new(r"\$\{(?P<var>[A-Z0-9_]+)(?::(?P<op>[-+])(?P<val>[^}]+))?\}").unwrap();
    re.replace_all(yaml, |caps: &regex::Captures| {
        let var = &caps["var"];
        match caps.name("op").map(|m| m.as_str()) {
            Some("-") => {
                let default = &caps["val"];
                env.get(var).cloned().unwrap_or_else(|| default.to_string())
            }
            Some("+") => {
                let value = &caps["val"];
                if env.contains_key(var) {
                    value.to_string()
                } else {
                    "".to_string()
                }
            }
            _ => env.get(var).cloned().unwrap_or_default(),
        }
    }).to_string()
}

pub fn parse_dotenv(content: &str) -> HashMap<String, String> {
    let mut env = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            env.insert(k.trim().to_string(), v.trim().trim_matches('"').to_string());
        }
    }
    env
}

/// Load a .env file and merge with process environment.
pub fn load_env(project_dir: &Path, extra_env_files: &[PathBuf]) -> HashMap<String, String> {
    let mut env: HashMap<String, String> = std::env::vars().collect();

    let default_env = project_dir.join(".env");
    if default_env.exists() {
        if let Ok(content) = std::fs::read_to_string(&default_env) {
            for (k, v) in parse_dotenv(&content) {
                env.entry(k).or_insert(v);
            }
        }
    }

    for ef in extra_env_files {
        if let Ok(content) = std::fs::read_to_string(ef) {
            for (k, v) in parse_dotenv(&content) {
                env.insert(k, v);
            }
        }
    }

    env
}

/// Parse a compose YAML string into a ComposeSpec after interpolation.
pub fn parse_compose_yaml(yaml: &str, env: &HashMap<String, String>) -> Result<ComposeSpec> {
    let interpolated = interpolate(yaml, env);
    serde_yaml::from_str(&interpolated).map_err(ComposeError::ParseError)
}

pub fn parse_and_merge_files(files: &[PathBuf], env: &HashMap<String, String>) -> Result<ComposeSpec> {
    let mut merged = ComposeSpec::default();
    for file in files {
        let content = std::fs::read_to_string(file)
            .map_err(|_| ComposeError::FileNotFound { path: file.display().to_string() })?;
        let spec = parse_compose_yaml(&content, env)?;
        if merged.services.is_empty() {
            merged = spec;
        } else {
            merged.merge(spec);
        }
    }
    Ok(merged)
}
