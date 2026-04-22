use crate::error::Result;
use async_trait::async_trait;
use crate::backend::ContainerBackend;

#[async_trait]
pub trait ContainerCommand: Send + Sync {
    async fn exec(&self, backend: &dyn ContainerBackend) -> Result<()>;
}

pub mod build;
pub mod run;
pub mod start;
pub mod stop;
pub mod inspect;
