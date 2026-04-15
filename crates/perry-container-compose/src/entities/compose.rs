//! Compose entity — root compose-spec structure.

use crate::entities::service::Service;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============ Top-level Network ============

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeNetworkIpamConfig {
    pub subnet: Option<String>,
    pub ip_range: Option<String>,
    pub gateway: Option<String>,
    pub aux_addresses: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeNetworkIpam {
    pub driver: Option<String>,
    pub config: Option<Vec<ComposeNetworkIpamConfig>>,
    pub options: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeNetwork {
    pub name: Option<String>,
    pub driver: Option<String>,
    pub driver_opts: Option<HashMap<String, String>>,
    pub ipam: Option<ComposeNetworkIpam>,
    pub external: Option<bool>,
    pub internal: Option<bool>,
    pub enable_ipv4: Option<bool>,
    pub enable_ipv6: Option<bool>,
    pub attachable: Option<bool>,
    pub labels: Option<crate::entities::service::ListOrDict>,
}

// ============ Top-level Volume ============

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeVolume {
    pub name: Option<String>,
    pub driver: Option<String>,
    pub driver_opts: Option<HashMap<String, String>>,
    pub external: Option<bool>,
    pub labels: Option<crate::entities::service::ListOrDict>,
}

// ============ Top-level Secret ============

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeSecret {
    pub name: Option<String>,
    pub environment: Option<String>,
    pub file: Option<String>,
    pub external: Option<bool>,
    pub labels: Option<crate::entities::service::ListOrDict>,
    pub driver: Option<String>,
    pub driver_opts: Option<HashMap<String, String>>,
    pub template_driver: Option<String>,
}

// ============ Top-level Config ============

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeConfig {
    pub name: Option<String>,
    pub content: Option<String>,
    pub environment: Option<String>,
    pub file: Option<String>,
    pub external: Option<bool>,
    pub labels: Option<crate::entities::service::ListOrDict>,
    pub template_driver: Option<String>,
}

// ============ Root Compose struct ============

/// Root compose-spec document.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Compose {
    /// Stack name
    pub name: Option<String>,

    /// Compose file version (ignored for validation, kept for compatibility)
    pub version: Option<String>,

    /// Service definitions
    #[serde(default)]
    pub services: HashMap<String, Service>,

    /// Top-level network definitions
    #[serde(default)]
    pub networks: Option<HashMap<String, ComposeNetwork>>,

    /// Top-level volume definitions
    #[serde(default)]
    pub volumes: Option<HashMap<String, ComposeVolume>>,

    /// Top-level secret definitions
    #[serde(default)]
    pub secrets: Option<HashMap<String, ComposeSecret>>,

    /// Top-level config definitions
    #[serde(default)]
    pub configs: Option<HashMap<String, ComposeConfig>>,

    /// Included compose files (compose-spec extension)
    pub include: Option<Vec<serde_json::Value>>,

    /// AI model definitions (compose-spec extension)
    pub models: Option<HashMap<String, serde_json::Value>>,
}

impl Compose {
    /// Parse from raw YAML bytes.
    pub fn parse(yaml: &[u8]) -> Result<Self> {
        let compose: Compose = serde_yaml::from_slice(yaml)?;
        Ok(compose)
    }

    /// Parse from a YAML string.
    pub fn parse_str(yaml: &str) -> Result<Self> {
        let compose: Compose = serde_yaml::from_str(yaml)?;
        Ok(compose)
    }

    /// Serialise to YAML.
    pub fn to_yaml(&self) -> Result<String> {
        Ok(serde_yaml::to_string(self)?)
    }

    /// Merge another Compose into this one (later values override earlier).
    pub fn merge(&mut self, other: Compose) {
        for (name, service) in other.services {
            self.services.insert(name, service);
        }

        if let Some(nets) = other.networks {
            let existing = self.networks.get_or_insert_with(HashMap::new);
            for (name, net) in nets {
                existing.insert(name, net);
            }
        }

        if let Some(vols) = other.volumes {
            let existing = self.volumes.get_or_insert_with(HashMap::new);
            for (name, vol) in vols {
                existing.insert(name, vol);
            }
        }

        if let Some(secs) = other.secrets {
            let existing = self.secrets.get_or_insert_with(HashMap::new);
            for (name, sec) in secs {
                existing.insert(name, sec);
            }
        }

        if let Some(cfgs) = other.configs {
            let existing = self.configs.get_or_insert_with(HashMap::new);
            for (name, cfg) in cfgs {
                existing.insert(name, cfg);
            }
        }

        if other.name.is_some() {
            self.name = other.name;
        }
        if other.version.is_some() {
            self.version = other.version;
        }
    }
}
