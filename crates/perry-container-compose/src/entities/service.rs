//! Service entity — mirrors internal/entities/service.go

use crate::error::Result;
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Build configuration for a service image
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Build {
    /// Build context directory (relative to compose file)
    pub context: Option<String>,
    /// Path to Dockerfile (relative to context)
    #[serde(default)]
    pub dockerfile: Option<String>,
    /// Build-time variables
    #[serde(default)]
    pub args: Option<HashMap<String, String>>,
    /// Labels to add to the built image
    #[serde(default)]
    pub labels: Option<HashMap<String, String>>,
    /// Build target stage
    #[serde(default)]
    pub target: Option<String>,
    /// Network to use during build
    #[serde(default)]
    pub network: Option<String>,
}

/// Restart policy variants
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RestartPolicy {
    No,
    Always,
    OnFailure,
    UnlessStopped,
}

impl Default for RestartPolicy {
    fn default() -> Self {
        RestartPolicy::No
    }
}

/// A single service definition in a Compose file.
///
/// Field names match Docker Compose YAML conventions.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Service {
    /// Container image reference (mutually exclusive with `build` for runtime,
    /// but both can coexist when building locally)
    pub image: Option<String>,

    /// Explicit container name; if absent, one is generated
    #[serde(rename = "container_name", default)]
    pub name: Option<String>,

    /// Port mappings, e.g. `"8080:80"`
    #[serde(default)]
    pub ports: Option<Vec<String>>,

    /// Environment variables
    #[serde(default)]
    pub environment: Option<EnvironmentSpec>,

    /// Container labels
    #[serde(default)]
    pub labels: Option<HashMap<String, String>>,

    /// Volume mounts, e.g. `"./data:/data:ro"`
    #[serde(default)]
    pub volumes: Option<Vec<String>>,

    /// Build configuration (optional; overrides `image` as the source)
    #[serde(default)]
    pub build: Option<Build>,

    /// Service dependencies
    #[serde(default)]
    pub depends_on: Option<DependsOn>,

    /// Restart policy
    #[serde(default)]
    pub restart: Option<String>,

    /// Override container entrypoint
    #[serde(default)]
    pub entrypoint: Option<StringOrList>,

    /// Override container command
    #[serde(default)]
    pub command: Option<StringOrList>,

    /// Networks this service is attached to
    #[serde(default)]
    pub networks: Option<Vec<String>>,
}

/// Environment can be a key=value list or a map
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EnvironmentSpec {
    Map(HashMap<String, String>),
    List(Vec<String>),
}

impl EnvironmentSpec {
    /// Resolve to a flat HashMap
    pub fn to_map(&self) -> HashMap<String, String> {
        match self {
            EnvironmentSpec::Map(m) => m.clone(),
            EnvironmentSpec::List(list) => list
                .iter()
                .filter_map(|entry| {
                    let mut parts = entry.splitn(2, '=');
                    let key = parts.next()?.to_owned();
                    let val = parts.next().unwrap_or("").to_owned();
                    Some((key, val))
                })
                .collect(),
        }
    }
}

/// String or list of strings (used for command/entrypoint)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StringOrList {
    String(String),
    List(Vec<String>),
}

impl StringOrList {
    pub fn to_list(&self) -> Vec<String> {
        match self {
            StringOrList::String(s) => vec![s.clone()],
            StringOrList::List(l) => l.clone(),
        }
    }
}

/// `depends_on` can be a list of service names or a map with conditions
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DependsOn {
    List(Vec<String>),
    Map(HashMap<String, DependsOnCondition>),
}

impl DependsOn {
    /// Return service names this service depends on (conditions ignored for now)
    pub fn service_names(&self) -> Vec<String> {
        match self {
            DependsOn::List(names) => names.clone(),
            DependsOn::Map(map) => map.keys().cloned().collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependsOnCondition {
    pub condition: Option<String>,
}

// ============ Service Methods ============

impl Service {
    /// Generate a unique container name based on service name and image.
    ///
    /// Mirrors the Go `generate_name` function using MD5.
    pub fn generate_name(&self, service_name: &str) -> Result<String> {
        if let Some(explicit) = &self.name {
            return Ok(explicit.clone());
        }

        let image = self
            .image
            .as_deref()
            .unwrap_or(service_name);

        let mut hasher = Md5::new();
        hasher.update(image.as_bytes());
        let hash = hasher.finalize();
        let hash_str = hex::encode(hash);
        // Take first 8 hex chars for brevity
        let prefix = &hash_str[..8];

        // Sanitise service name: replace non-alphanumeric with _
        let safe_name: String = service_name
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
            .collect();

        Ok(format!("{}_{}", safe_name, prefix))
    }

    /// Whether the service needs to build an image before running
    pub fn needs_build(&self) -> bool {
        self.build.is_some() && self.image.is_none()
    }

    /// Return the image tag to use for this service (image name or generated tag)
    pub fn image_ref(&self, service_name: &str) -> String {
        if let Some(image) = &self.image {
            return image.clone();
        }
        // For build-only services we use the service name as the local tag
        format!("{}-image", service_name)
    }

    /// Get resolved environment variables as a HashMap
    pub fn resolved_env(&self) -> HashMap<String, String> {
        self.environment
            .as_ref()
            .map(|e| e.to_map())
            .unwrap_or_default()
    }
}
