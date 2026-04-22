use async_trait::async_trait;
use crate::commands::ContainerCommand;
use crate::error::Result;
use crate::backend::ContainerBackend;
use crate::types::ContainerSpec;

pub struct RunCommand<'a> {
    pub backend: &'a dyn ContainerBackend,
    pub spec: &'a ContainerSpec,
}

#[async_trait]
impl<'a> ContainerCommand for RunCommand<'a> {
    async fn exec(&self) -> Result<()> {
        self.backend.run(self.spec).await?;
        Ok(())
    }
}
