//! YAML parsing, env interpolation, .env loading, multi-file merge.

use crate::error::{ComposeError, Result};
use crate::types::ComposeSpec;
use regex::{Captures, Regex};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Interpolate ${VAR}, ${VAR:-default}, ${VAR:+value} in a YAML string.
pub fn interpolate_yaml(yaml: &str, env: &HashMap<String, String>) -> String {
    let re = Regex::new(r"\$\{(?P<name>[A-Z0-9_]+)(?::(?P<op>[-+])(?P<value>[^}]*))?\}").unwrap();

    re.replace_all(yaml, |caps: &Captures| {
        let name = caps.name("name").unwrap().as_str();
        let op = caps.name("op").map(|m| m.as_str());
        let val = caps.name("value").map(|m| m.as_str()).unwrap_or("");

        match op {
            Some("-") => {
                // ${VAR:-default} -> use default if VAR is missing or empty
                env.get(name)
                    .filter(|s| !s.is_empty())
                    .cloned()
                    .unwrap_or_else(|| val.to_string())
            }
            Some("+") => {
                // ${VAR:+value} -> use value if VAR is present and not empty, else empty
                env.get(name)
                    .filter(|s| !s.is_empty())
                    .map(|_| val.to_string())
                    .unwrap_or_default()
            }
            _ => {
                // ${VAR} -> use VAR or empty
                env.get(name).cloned().unwrap_or_default()
            }
        }
    })
    .to_string()
}

/// Load a .env file and merge with process environment.
pub fn load_env(project_dir: &Path, extra_env_files: &[PathBuf]) -> HashMap<String, String> {
    let mut env: HashMap<String, String> = std::env::vars().collect();

    // Default .env
    let default_env = project_dir.join(".env");
    if default_env.exists() {
        if let Ok(content) = std::fs::read_to_string(&default_env) {
            for (k, v) in parse_dotenv(&content) {
                env.entry(k).or_insert(v);
            }
        }
    }

    // Extra env files
    for path in extra_env_files {
        if let Ok(content) = std::fs::read_to_string(path) {
            for (k, v) in parse_dotenv(&content) {
                env.insert(k, v);
            }
        }
    }

    env
}

fn parse_dotenv(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=') {
            let val = v.trim().trim_matches('"').trim_matches('\'');
            map.insert(k.trim().to_string(), val.to_string());
        }
    }
    map
}

/// Parse a compose YAML string into a ComposeSpec after interpolation.
pub fn parse_compose_yaml(yaml: &str, env: &HashMap<String, String>) -> Result<ComposeSpec> {
    let interpolated = interpolate_yaml(yaml, env);
    ComposeSpec::parse_str(&interpolated)
}

/// Read, interpolate, parse, and merge multiple compose files in order.
pub fn parse_and_merge_files(files: &[PathBuf], env: &HashMap<String, String>) -> Result<ComposeSpec> {
    if files.is_empty() {
        return Err(ComposeError::validation("No compose files provided"));
    }

    let mut merged = ComposeSpec::default();
    for path in files {
        let content = std::fs::read_to_string(path).map_err(|_| ComposeError::FileNotFound {
            path: path.display().to_string(),
        })?;
        let spec = parse_compose_yaml(&content, env)?;
        if merged.services.is_empty() {
            merged = spec;
        } else {
            merged.merge(spec);
        }
    }

    Ok(merged)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolate_simple() {
        let mut env = HashMap::new();
        env.insert("TAG".into(), "v1".into());
        let res = interpolate_yaml("image: nginx:${TAG}", &env);
        assert_eq!(res, "image: nginx:v1");
    }

    #[test]
    fn test_interpolate_default() {
        let env = HashMap::new();
        let res = interpolate_yaml("image: nginx:${TAG:-latest}", &env);
        assert_eq!(res, "image: nginx:latest");
    }

    #[test]
    fn test_parse_dotenv() {
        let content = "FOO=bar\n# comment\nBAZ=qux ";
        let env = parse_dotenv(content);
        assert_eq!(env.get("FOO"), Some(&"bar".to_string()));
        assert_eq!(env.get("BAZ"), Some(&"qux".to_string()));
    }
}
