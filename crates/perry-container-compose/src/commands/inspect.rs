use async_trait::async_trait;
use crate::error::Result;
use crate::backend::ContainerBackend;
use crate::types::ContainerInfo;
use super::ContainerCommand;

pub struct InspectCommand {
    pub id: String,
}

#[async_trait]
impl ContainerCommand for InspectCommand {
    async fn exec(&self, backend: &dyn ContainerBackend) -> Result<()> {
        backend.inspect(&self.id).await.map(|_| ())
    }
}
