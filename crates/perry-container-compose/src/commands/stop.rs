use crate::error::Result;
use crate::backend::ContainerBackend;
use crate::service::Service;
use crate::commands::ContainerCommand;
use async_trait::async_trait;

pub struct StopCommand {
    pub service_name: String,
    pub service: Service,
}

#[async_trait]
impl ContainerCommand for StopCommand {
    async fn exec(&self, backend: &dyn ContainerBackend) -> Result<()> {
        let name = self.service.name(&self.service_name);
        backend.stop(&name, None).await
    }
}
