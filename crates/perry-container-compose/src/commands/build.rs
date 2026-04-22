use crate::error::Result;
use crate::backend::ContainerBackend;
use crate::service::Service;
use crate::commands::ContainerCommand;
use async_trait::async_trait;

pub struct BuildCommand {
    pub service: Service,
}

#[async_trait]
impl ContainerCommand for BuildCommand {
    async fn exec(&self, backend: &dyn ContainerBackend) -> Result<()> {
        self.service.build_command(backend).await
    }
}
