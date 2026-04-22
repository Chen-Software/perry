use async_trait::async_trait;
use crate::commands::ContainerCommand;
use crate::error::Result;
use crate::backend::ContainerBackend;
use crate::types::ContainerInfo;

pub struct InspectCommand<'a> {
    pub backend: &'a dyn ContainerBackend,
    pub id: String,
}

#[async_trait]
impl<'a> ContainerCommand for InspectCommand<'a> {
    async fn exec(&self) -> Result<()> {
        self.backend.inspect(&self.id).await?;
        Ok(())
    }
}

impl<'a> InspectCommand<'a> {
    pub async fn get_info(&self) -> Result<ContainerInfo> {
        self.backend.inspect(&self.id).await
    }
}
