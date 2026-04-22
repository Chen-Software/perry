use async_trait::async_trait;
use crate::error::Result;
use crate::backend::ContainerBackend;
use super::ContainerCommand;

pub struct StartCommand {
    pub id: String,
}

#[async_trait]
impl ContainerCommand for StartCommand {
    async fn exec(&self, backend: &dyn ContainerBackend) -> Result<()> {
        backend.start(&self.id).await
    }
}
