use async_trait::async_trait;
use crate::error::Result;
use crate::backend::ContainerBackend;
use crate::types::Container;
use super::ContainerCommand;

pub struct RunCommand {
    pub spec: Container,
}

#[async_trait]
impl ContainerCommand for RunCommand {
    async fn exec(&self, backend: &dyn ContainerBackend) -> Result<()> {
        backend.run(&self.spec).await.map(|_| ())
    }
}
