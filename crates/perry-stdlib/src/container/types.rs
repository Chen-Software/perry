//! Type definitions for the perry/container module.

use perry_runtime::StringHeader;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::common::handle::{register_handle, Handle};

use perry_container_compose::ComposeEngine;

// ============ Handle Registry ============

pub struct ContainerHandle {
    pub id: String,
    pub name: Option<String>,
}

pub fn register_container_handle(handle: ContainerHandle) -> Handle {
    register_handle(handle)
}

pub fn register_compose_handle(engine: ComposeEngine) -> Handle {
    register_handle(std::sync::Arc::new(engine))
}

// ============ Core Container Types ============

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContainerSpec {
    pub image: String,
    pub name: Option<String>,
    pub ports: Option<Vec<String>>,
    pub volumes: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub cmd: Option<Vec<String>>,
    pub entrypoint: Option<Vec<String>>,
    pub network: Option<String>,
    pub rm: Option<bool>,
    pub read_only: Option<bool>,
    pub seccomp: Option<String>,
    pub labels: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub ports: Vec<String>,
    pub created: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

// ============ Helper for StringHeader ============

pub unsafe fn string_from_header(header: *const StringHeader) -> Option<String> {
    if header.is_null() || (header as usize) < 0x1000 {
        return None;
    }
    let s = (*header).as_str();
    Some(s.to_string())
}
