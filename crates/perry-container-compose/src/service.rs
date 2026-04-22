use md5::{Digest, Md5};
use crate::types::{ComposeService, ContainerSpec, ListOrDict};
use crate::backend::ContainerBackend;
use crate::error::Result;
use std::collections::HashMap;

pub fn generate_name(image: &str, service_name: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(image.as_bytes());
    let hash = hasher.finalize();
    let short_hash = &hex::encode(hash)[..8];

    let random_suffix: u32 = rand::random();

    let safe_name: String = service_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
        .collect();

    format!("{}-{}-{:08x}", safe_name, short_hash, random_suffix)
}

impl ComposeService {
    pub fn name(&self, service_key: &str) -> String {
        if let Some(name) = &self.container_name {
            return name.clone();
        }
        generate_name(self.image.as_deref().unwrap_or("unknown"), service_key)
    }

    pub async fn is_running(&self, service_key: &str, backend: &dyn ContainerBackend) -> Result<bool> {
        let name = self.name(service_key);
        match backend.inspect(&name).await {
            Ok(info) => Ok(info.status.to_lowercase().contains("running") || info.status.to_lowercase().contains("up")),
            Err(_) => Ok(false),
        }
    }

    pub async fn exists(&self, service_key: &str, backend: &dyn ContainerBackend) -> Result<bool> {
        let name = self.name(service_key);
        Ok(backend.inspect(&name).await.is_ok())
    }

    pub async fn start_command(&self, service_key: &str, backend: &dyn ContainerBackend) -> Result<()> {
        let name = self.name(service_key);
        backend.start(&name).await
    }

    pub async fn run_command(&self, service_key: &str, backend: &dyn ContainerBackend) -> Result<()> {
        let name = self.name(service_key);
        let mut spec = ContainerSpec {
            image: self.image.clone().unwrap_or_default(),
            name: Some(name),
            rm: Some(false),
            ..Default::default()
        };

        if let Some(ports) = &self.ports {
            spec.ports = Some(ports.iter().map(|p| match p {
                crate::types::PortSpec::Short(v) => {
                    if let Some(s) = v.as_str() {
                        s.to_string()
                    } else if let Some(i) = v.as_i64() {
                        i.to_string()
                    } else {
                        "".to_string()
                    }
                },
                crate::types::PortSpec::Long(lp) => {
                    let published = lp.published.as_ref().map(|v| {
                        if let Some(s) = v.as_str() {
                            s.to_string()
                        } else if let Some(i) = v.as_i64() {
                            i.to_string()
                        } else {
                            "".to_string()
                        }
                    }).unwrap_or_default();
                    let target = if let Some(s) = lp.target.as_str() {
                        s.to_string()
                    } else if let Some(i) = lp.target.as_i64() {
                        i.to_string()
                    } else {
                        "".to_string()
                    };
                    if published.is_empty() {
                        target
                    } else {
                        format!("{}:{}", published, target)
                    }
                },
            }).collect());
        }

        if let Some(volumes) = &self.volumes {
            spec.volumes = Some(volumes.iter().filter_map(|v| v.as_str()).map(|s| s.to_string()).collect());
        }

        if let Some(env) = &self.environment {
            let mut env_map = HashMap::new();
            match env {
                ListOrDict::Dict(d) => {
                    for (k, v) in d {
                        env_map.insert(k.clone(), v.as_ref().and_then(|val| {
                            if let Some(s) = val.as_str() {
                                Some(s.to_string())
                            } else if let Some(i) = val.as_i64() {
                                Some(i.to_string())
                            } else if let Some(b) = val.as_bool() {
                                Some(b.to_string())
                            } else {
                                None
                            }
                        }).unwrap_or_default());
                    }
                }
                ListOrDict::List(l) => {
                    for s in l {
                        if let Some((k, v)) = s.split_once('=') {
                            env_map.insert(k.to_string(), v.to_string());
                        }
                    }
                }
            }
            spec.env = Some(env_map);
        }

        if let Some(cmd) = &self.command {
            spec.cmd = match cmd {
                serde_yaml::Value::String(s) => Some(s.split_whitespace().map(|x| x.to_string()).collect()),
                serde_yaml::Value::Sequence(seq) => Some(seq.iter().filter_map(|v| {
                    if let Some(s) = v.as_str() {
                        Some(s.to_string())
                    } else if let Some(i) = v.as_i64() {
                        Some(i.to_string())
                    } else {
                        None
                    }
                }).collect()),
                _ => None,
            };
        }

        if let Some(networks) = &self.networks {
            match networks {
                crate::types::ServiceNetworks::List(l) => {
                    if let Some(first) = l.first() {
                        spec.network = Some(first.clone());
                    }
                }
                crate::types::ServiceNetworks::Map(m) => {
                    if let Some(first) = m.keys().next() {
                        spec.network = Some(first.clone());
                    }
                }
            }
        }

        backend.run(&spec).await.map(|_| ())
    }
}
