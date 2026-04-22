use crate::error::{ComposeError, Result};
use crate::types::ComposeSpec;
use regex::{Captures, Regex};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub fn interpolate_yaml(yaml: &str, env: &HashMap<String, String>) -> String {
    let re = Regex::new(r"\$\{(?P<var>[A-Z0-9_]+)(?::-(?P<def>.*?))?\}").unwrap();
    re.replace_all(yaml, |caps: &Captures| {
        let var = &caps["var"];
        env.get(var).cloned().unwrap_or_else(|| {
            caps.name("def")
                .map(|m| m.as_str().to_string())
                .unwrap_or_default()
        })
    })
    .into_owned()
}

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

fn parse_dotenv(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            map.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    map
}

pub fn parse_compose_yaml(yaml: &str, env: &HashMap<String, String>) -> Result<ComposeSpec> {
    let interpolated = interpolate_yaml(yaml, env);
    serde_yaml::from_str(&interpolated).map_err(ComposeError::ParseError)
}

pub fn parse_and_merge_files(files: &[PathBuf], env: &HashMap<String, String>) -> Result<ComposeSpec> {
    let mut merged = ComposeSpec::default();
    for f in files {
        if !f.exists() {
            return Err(ComposeError::FileNotFound { path: f.to_string_lossy().to_string() });
        }
        let content = std::fs::read_to_string(f).map_err(ComposeError::IoError)?;
        let spec = parse_compose_yaml(&content, env)?;
        merged.merge(spec);
    }
    Ok(merged)
}

impl ComposeSpec {
    pub fn merge(&mut self, other: ComposeSpec) {
        for (name, service) in other.services {
            self.services.insert(name, service);
        }
        if let Some(networks) = other.networks {
            let self_networks = self.networks.get_or_insert_with(Default::default);
            for (name, config) in networks {
                self_networks.insert(name, config);
            }
        }
        if let Some(volumes) = other.volumes {
            let self_volumes = self.volumes.get_or_insert_with(Default::default);
            for (name, config) in volumes {
                self_volumes.insert(name, config);
            }
        }
        if let Some(secrets) = other.secrets {
            let self_secrets = self.secrets.get_or_insert_with(Default::default);
            for (name, config) in secrets {
                self_secrets.insert(name, config);
            }
        }
        if let Some(configs) = other.configs {
            let self_configs = self.configs.get_or_insert_with(Default::default);
            for (name, config) in configs {
                self_configs.insert(name, config);
            }
        }
        self.extensions.extend(other.extensions);
    }
}
