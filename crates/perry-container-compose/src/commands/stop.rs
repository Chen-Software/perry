use crate::error::Result;
use crate::backend::ContainerBackend;
use crate::service::Service;
use crate::commands::ContainerCommand;
use async_trait::async_trait;

pub struct StopCommand<'a> {
    pub service: &'a Service,
}

#[async_trait]
impl<'a> ContainerCommand for StopCommand<'a> {
    async fn exec(&self, backend: &dyn ContainerBackend) -> Result<()> {
        let name = self.service.name();
        let containers = backend.list(false).await?;
        if let Some(c) = containers.iter().find(|c| c.name == name || c.name.contains(&name)) {
            backend.stop(&c.id, None).await
        } else {
            Ok(()) // Already stopped or not found
        }
    }
}
