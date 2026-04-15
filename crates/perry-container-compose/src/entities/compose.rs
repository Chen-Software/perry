//! Compose entity — mirrors internal/entities/compose.go

use crate::entities::service::Service;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Network definition in a Compose file
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeNetwork {
    pub driver: Option<String>,
    pub external: Option<bool>,
    pub name: Option<String>,
}

/// Volume definition in a Compose file
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeVolume {
    pub driver: Option<String>,
    pub external: Option<bool>,
    pub name: Option<String>,
}

/// Root Compose file structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Compose {
    /// Compose file version (ignored for validation, kept for compatibility)
    pub version: Option<String>,

    /// Service definitions
    #[serde(default)]
    pub services: HashMap<String, Service>,

    /// Named network definitions
    #[serde(default)]
    pub networks: Option<HashMap<String, ComposeNetwork>>,

    /// Named volume definitions
    #[serde(default)]
    pub volumes: Option<HashMap<String, ComposeVolume>>,
}

impl Compose {
    /// Parse a Compose struct from raw YAML bytes
    pub fn parse(yaml: &[u8]) -> Result<Self> {
        let compose: Compose = serde_yaml::from_slice(yaml)?;
        Ok(compose)
    }

    /// Parse a Compose struct from a YAML string
    pub fn parse_str(yaml: &str) -> Result<Self> {
        let compose: Compose = serde_yaml::from_str(yaml)?;
        Ok(compose)
    }

    /// Serialise the compose spec back to YAML
    pub fn to_yaml(&self) -> Result<String> {
        Ok(serde_yaml::to_string(self)?)
    }

    /// Merge another Compose into this one (later values override earlier)
    pub fn merge(&mut self, other: Compose) {
        // Services: other overrides self
        for (name, service) in other.services {
            self.services.insert(name, service);
        }

        // Networks
        if let Some(nets) = other.networks {
            let existing = self.networks.get_or_insert_with(HashMap::new);
            for (name, net) in nets {
                existing.insert(name, net);
            }
        }

        // Volumes
        if let Some(vols) = other.volumes {
            let existing = self.volumes.get_or_insert_with(HashMap::new);
            for (name, vol) in vols {
                existing.insert(name, vol);
            }
        }

        // Version: use whichever is set; prefer other
        if other.version.is_some() {
            self.version = other.version;
        }
    }
}
