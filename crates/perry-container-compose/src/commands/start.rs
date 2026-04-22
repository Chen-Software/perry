use async_trait::async_trait;
use crate::commands::ContainerCommand;
use crate::error::Result;
use crate::backend::ContainerBackend;

pub struct StartCommand<'a> {
    pub backend: &'a dyn ContainerBackend,
    pub id: String,
}

#[async_trait]
impl<'a> ContainerCommand for StartCommand<'a> {
    async fn exec(&self) -> Result<()> {
        self.backend.start(&self.id).await
    }
}
