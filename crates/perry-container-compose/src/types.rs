//! Data types for perry-container-compose.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeSpec {
    pub version: Option<String>,
    pub name: Option<String>,
    pub services: IndexMap<String, ComposeService>,
    pub networks: Option<IndexMap<String, Option<ComposeNetwork>>>,
    pub volumes: Option<IndexMap<String, Option<ComposeVolume>>>,
}

impl ComposeSpec {
    pub fn to_yaml(&self) -> Result<String, crate::error::ComposeError> {
        serde_yaml::to_string(self).map_err(crate::error::ComposeError::ParseError)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeService {
    pub image: Option<String>,
    pub build: Option<ComposeServiceBuild>,
    pub command: Option<serde_yaml::Value>,
    pub entrypoint: Option<serde_yaml::Value>,
    pub environment: Option<ListOrDict>,
    pub ports: Option<Vec<String>>,
    pub volumes: Option<Vec<String>>,
    pub networks: Option<Vec<String>>,
    pub depends_on: Option<DependsOnSpec>,
    pub container_name: Option<String>,
    pub labels: Option<ListOrDict>,
}

impl ComposeService {
    pub fn image_ref(&self, svc_name: &str) -> String {
        self.image.clone().unwrap_or_else(|| svc_name.to_string())
    }
    pub fn port_strings(&self) -> Vec<String> { self.ports.clone().unwrap_or_default() }
    pub fn volume_strings(&self) -> Vec<String> { self.volumes.clone().unwrap_or_default() }
    pub fn resolved_env(&self) -> HashMap<String, String> {
        match &self.environment {
            Some(ListOrDict::Dict(m)) => m.iter().filter_map(|(k, v)| v.as_ref().map(|val| (k.clone(), val.to_string()))).collect(),
            Some(ListOrDict::List(l)) => l.iter().filter_map(|s| {
                let mut p = s.splitn(2, '=');
                Some((p.next()?.to_string(), p.next()?.to_string()))
            }).collect(),
            None => HashMap::new(),
        }
    }
    pub fn command_list(&self) -> Option<Vec<String>> {
        match &self.command {
            Some(serde_yaml::Value::String(s)) => Some(s.split_whitespace().map(|s| s.to_string()).collect()),
            Some(serde_yaml::Value::Sequence(seq)) => Some(seq.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeServiceBuild {
    pub context: Option<String>,
    pub dockerfile: Option<String>,
    pub args: Option<ListOrDict>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ListOrDict {
    Dict(IndexMap<String, Option<serde_yaml::Value>>),
    List(Vec<String>),
}

impl ListOrDict {
    pub fn to_map(&self) -> HashMap<String, String> {
        match self {
            Self::Dict(m) => m.iter().filter_map(|(k, v)| v.as_ref().map(|val| (k.clone(), val.to_string()))).collect(),
            Self::List(l) => l.iter().filter_map(|s| {
                let mut p = s.splitn(2, '=');
                Some((p.next()?.to_string(), p.next()?.to_string()))
            }).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DependsOnSpec {
    List(Vec<String>),
    Map(IndexMap<String, serde_json::Value>),
}

impl DependsOnSpec {
    pub fn service_names(&self) -> Vec<String> {
        match self {
            Self::List(l) => l.clone(),
            Self::Map(m) => m.keys().cloned().collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeNetwork {
    pub driver: Option<String>,
    pub external: Option<bool>,
    pub internal: Option<bool>,
    pub enable_ipv6: Option<bool>,
    pub name: Option<String>,
    pub labels: Option<ListOrDict>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeVolume {
    pub driver: Option<String>,
    pub external: Option<bool>,
    pub name: Option<String>,
    pub labels: Option<ListOrDict>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContainerSpec {
    pub image: String,
    pub name: Option<String>,
    pub ports: Option<Vec<String>>,
    pub volumes: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub labels: Option<HashMap<String, String>>,
    pub cmd: Option<Vec<String>>,
    pub entrypoint: Option<Vec<String>>,
    pub network: Option<String>,
    pub rm: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerHandle {
    pub id: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub ports: Vec<String>,
    pub labels: HashMap<String, String>,
    pub created: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerLogs {
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInfo {
    pub id: String,
    pub repository: String,
    pub tag: String,
    pub size: u64,
    pub created: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeHandle {
    pub stack_id: u64,
    pub project_name: String,
    pub services: Vec<String>,
}
