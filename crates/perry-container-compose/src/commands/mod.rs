pub mod build;
pub mod inspect;
pub mod run;
pub mod start;
pub mod stop;

use crate::error::Result;
use crate::backend::ContainerBackend;
use async_trait::async_trait;

#[async_trait]
pub trait ContainerCommand: Send + Sync {
    async fn exec(&self, backend: &dyn ContainerBackend) -> Result<()>;
}
