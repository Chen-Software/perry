use std::collections::HashMap;
use std::path::Path;
use serde_yaml::Value;
use crate::error::ComposeError;
use crate::types::ComposeSpec;

pub fn parse_yaml(content: &str) -> Result<ComposeSpec, ComposeError> {
    let interpolated = interpolate_env(content);
    serde_yaml::from_str(&interpolated).map_err(|e| ComposeError::InvalidConfig(e.to_string()))
}

pub fn interpolate_env(content: &str) -> String {
    let mut result = content.to_string();
    let re = regex::Regex::new(r"\$\{([A-Z0-9_]+)(?::-([^}]*))?\}").unwrap();

    // We use a simple replacement loop for now.
    // In production we'd want something more robust.
    loop {
        let current = result.clone();
        if let Some(caps) = re.captures(&current) {
            let var_name = &caps[1];
            let default_val = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            let val = std::env::var(var_name).unwrap_or_else(|_| default_val.to_string());
            result = current.replace(&caps[0], &val);
        } else {
            break;
        }
    }
    result
}

pub fn merge_specs(base: &mut ComposeSpec, override_spec: ComposeSpec) {
    for (name, service) in override_spec.services {
        base.services.insert(name, service);
    }
    if let Some(nets) = override_spec.networks {
        let base_nets = base.networks.get_or_insert_with(HashMap::new);
        for (name, net) in nets {
            base_nets.insert(name, net);
        }
    }
    if let Some(vols) = override_spec.volumes {
        let base_vols = base.volumes.get_or_insert_with(HashMap::new);
        for (name, vol) in vols {
            base_vols.insert(name, vol);
        }
    }
}
