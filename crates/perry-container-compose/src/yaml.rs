use std::collections::HashMap;
use std::path::{Path, PathBuf};
use regex::Regex;
use crate::error::{ComposeError, Result};
use crate::types::ComposeSpec;

pub fn interpolate_yaml(yaml: &str, env: &HashMap<String, String>) -> String {
    // Basic implementation of ${VAR}, ${VAR:-default}, ${VAR:+value}
    let re = Regex::new(r"\$\{([A-Za-z0-9_]+)(?::-([^}]+))?(?::\+([^}]+))?\}").unwrap();

    re.replace_all(yaml, |caps: &regex::Captures| {
        let var_name = &caps[1];
        let default_val = caps.get(2).map(|m| m.as_str());
        let alt_val = caps.get(3).map(|m| m.as_str());

        match env.get(var_name) {
            Some(val) if !val.is_empty() => {
                if let Some(alt) = alt_val {
                    alt.to_string()
                } else {
                    val.clone()
                }
            }
            _ => {
                if let Some(default) = default_val {
                    default.to_string()
                } else {
                    String::new()
                }
            }
        }
    }).to_string()
}

pub fn load_env(project_dir: &Path, extra_env_files: &[PathBuf]) -> HashMap<String, String> {
    let mut env = HashMap::new();

    // Load .env from project dir
    let dot_env = project_dir.join(".env");
    if dot_env.exists() {
        if let Ok(content) = std::fs::read_to_string(dot_env) {
            parse_env_file(&content, &mut env);
        }
    }

    // Load extra env files
    for file in extra_env_files {
        if let Ok(content) = std::fs::read_to_string(file) {
            parse_env_file(&content, &mut env);
        }
    }

    // Process env takes precedence
    for (k, v) in std::env::vars() {
        env.insert(k, v);
    }

    env
}

fn parse_env_file(content: &str, env: &mut HashMap<String, String>) {
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim().trim_matches('"').trim_matches('\'');
            env.insert(key.to_string(), value.to_string());
        }
    }
}

pub fn parse_compose_yaml(yaml: &str, env: &HashMap<String, String>) -> Result<ComposeSpec> {
    let interpolated = interpolate_yaml(yaml, env);
    serde_yaml::from_str(&interpolated).map_err(|e| ComposeError::ParseError(e.to_string()))
}

impl ComposeSpec {
    pub fn merge(&mut self, other: ComposeSpec) {
        // Last-writer-wins for all top-level maps
        for (k, v) in other.services {
            self.services.insert(k, v);
        }

        if let Some(networks) = other.networks {
            let base = self.networks.get_or_insert_with(Default::default);
            for (k, v) in networks {
                base.insert(k, v);
            }
        }

        if let Some(volumes) = other.volumes {
            let base = self.volumes.get_or_insert_with(Default::default);
            for (k, v) in volumes {
                base.insert(k, v);
            }
        }

        if let Some(secrets) = other.secrets {
            let base = self.secrets.get_or_insert_with(Default::default);
            for (k, v) in secrets {
                base.insert(k, v);
            }
        }

        if let Some(configs) = other.configs {
            let base = self.configs.get_or_insert_with(Default::default);
            for (k, v) in configs {
                base.insert(k, v);
            }
        }

        if let Some(name) = other.name {
            self.name = Some(name);
        }
    }
}

pub fn parse_and_merge_files(files: &[PathBuf], env: &HashMap<String, String>) -> Result<ComposeSpec> {
    if files.is_empty() {
        return Err(ComposeError::ValidationError("No compose files specified".to_string()));
    }

    let mut root_spec: Option<ComposeSpec> = None;

    for file in files {
        if !file.exists() {
            return Err(ComposeError::FileNotFound(file.to_string_lossy().to_string()));
        }
        let content = std::fs::read_to_string(file).map_err(|e| ComposeError::IoError(e.to_string()))?;
        let spec = parse_compose_yaml(&content, env)?;

        if let Some(mut root) = root_spec.take() {
            root.merge(spec);
            root_spec = Some(root);
        } else {
            root_spec = Some(spec);
        }
    }

    Ok(root_spec.unwrap())
}
