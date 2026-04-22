use async_trait::async_trait;
use crate::error::Result;

#[async_trait]
pub trait ContainerCommand: Send + Sync {
    async fn exec(&self) -> Result<()>;
}

pub mod build;
pub mod run;
pub mod start;
pub mod stop;
pub mod inspect;
