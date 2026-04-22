use crate::error::ComposeError;
use async_trait::async_trait;

#[async_trait]
pub trait ContainerCommand: Send + Sync {
    async fn exec(&self, backend: &dyn crate::backend::ContainerBackend) -> Result<(), ComposeError>;
}

pub mod build;
pub mod run;
pub mod start;
pub mod stop;
pub mod inspect;
