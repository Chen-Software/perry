use crate::error::Result;
use crate::backend::ContainerBackend;
use crate::service::Service;
use crate::commands::ContainerCommand;
use async_trait::async_trait;

pub struct InspectCommand {
    pub service: Service,
}

#[async_trait]
impl ContainerCommand for InspectCommand {
    async fn exec(&self, backend: &dyn ContainerBackend) -> Result<()> {
        self.service.inspect_command(backend).await.map(|_| ())
    }
}
