use async_trait::async_trait;
use crate::commands::ContainerCommand;
use crate::error::Result;
use crate::backend::ContainerBackend;

pub struct StopCommand<'a> {
    pub backend: &'a dyn ContainerBackend,
    pub id: String,
    pub timeout: Option<u32>,
}

#[async_trait]
impl<'a> ContainerCommand for StopCommand<'a> {
    async fn exec(&self) -> Result<()> {
        self.backend.stop(&self.id, self.timeout).await
    }
}
