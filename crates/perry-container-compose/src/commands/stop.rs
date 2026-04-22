use async_trait::async_trait;
use crate::error::Result;
use crate::backend::ContainerBackend;
use super::ContainerCommand;

pub struct StopCommand {
    pub id: String,
}

#[async_trait]
impl ContainerCommand for StopCommand {
    async fn exec(&self, backend: &dyn ContainerBackend) -> Result<()> {
        backend.stop(&self.id, None).await
    }
}
