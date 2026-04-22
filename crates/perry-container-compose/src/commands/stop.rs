use crate::error::Result;
use crate::backend::ContainerBackend;
use crate::service::Service;
use crate::commands::ContainerCommand;
use async_trait::async_trait;

pub struct StopCommand {
    pub service: Service,
}

#[async_trait]
impl ContainerCommand for StopCommand {
    async fn exec(&self, backend: &dyn ContainerBackend) -> Result<()> {
        let name = self.service.generate_name();
        backend.stop(&name, None).await
    }
}
