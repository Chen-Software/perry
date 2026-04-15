use crate::error::{ComposeError, Result};
use crate::types::ComposeSpec;
use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use once_cell::sync::Lazy;

static INTERPOLATION_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\$\{(?P<var>[A-Z0-9_]+)(?::(?P<op>[-+])(?P<val>[^}]+))?\}").unwrap()
});

/// Interpolate ${VAR}, ${VAR:-default}, ${VAR:+value} in a YAML string.
pub fn interpolate_yaml(yaml: &str, env: &HashMap<String, String>) -> String {
    INTERPOLATION_RE.replace_all(yaml, |caps: &regex::Captures| {
        let var_name = caps.name("var").unwrap().as_str();
        let op = caps.name("op").map(|m| m.as_str());
        let val = caps.name("val").map(|m| m.as_str()).unwrap_or("");

        match env.get(var_name) {
            Some(v) if !v.is_empty() => {
                if op == Some("+") {
                    val.to_string()
                } else {
                    v.clone()
                }
            }
            _ => {
                if op == Some("-") {
                    val.to_string()
                } else if op == Some("+") {
                    "".to_string()
                } else {
                    "".to_string()
                }
            }
        }
    }).to_string()
}

/// Load a .env file and merge with process environment.
pub fn load_env(project_dir: &Path, extra_env_files: &[PathBuf]) -> HashMap<String, String> {
    let mut env: HashMap<String, String> = std::env::vars().collect();

    let default_env = project_dir.join(".env");
    if default_env.exists() {
        if let Ok(content) = std::fs::read_to_string(&default_env) {
            parse_dotenv(&content, &mut env);
        }
    }

    for ef in extra_env_files {
        if let Ok(content) = std::fs::read_to_string(ef) {
            parse_dotenv(&content, &mut env);
        }
    }

    env
}

fn parse_dotenv(content: &str, env: &mut HashMap<String, String>) {
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let k = k.trim();
            let v = v.trim().trim_matches('"').trim_matches('\'');
            env.insert(k.to_string(), v.to_string());
        }
    }
}

/// Parse a compose YAML string into a ComposeSpec after interpolation.
pub fn parse_compose_yaml(yaml: &str, env: &HashMap<String, String>) -> Result<ComposeSpec> {
    let interpolated = interpolate_yaml(yaml, env);
    serde_yaml::from_str(&interpolated).map_err(ComposeError::ParseError)
}
