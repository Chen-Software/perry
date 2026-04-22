use crate::error::Result;
use crate::backend::ContainerBackend;
use crate::service::Service;
use crate::commands::ContainerCommand;
use async_trait::async_trait;

pub struct RunCommand {
    pub service_name: String,
    pub service: Service,
}

#[async_trait]
impl ContainerCommand for RunCommand {
    async fn exec(&self, backend: &dyn ContainerBackend) -> Result<()> {
        self.service.run_command(&self.service_name, backend).await
    }
}
