use std::collections::HashMap;

pub struct Config {
    pub project_name: Option<String>,
    pub env: HashMap<String, String>,
}

impl Config {
    pub fn load() -> Self {
        Config {
            project_name: std::env::var("COMPOSE_PROJECT_NAME").ok(),
            env: std::env::vars().collect(),
        }
    }
}
