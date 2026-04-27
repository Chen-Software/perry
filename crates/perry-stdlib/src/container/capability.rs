use crate::container::types::{ContainerLogs, ContainerError};

pub async fn alloy_container_run_capability(
    _name: &str,
    _image: &str,
    _cmd: &[&str],
) -> Result<ContainerLogs, ContainerError> {
    Err(ContainerError::Other("Not implemented".into()))
}
