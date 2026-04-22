use crate::backend::ContainerBackend;
use crate::commands::ContainerCommand;
use crate::error::Result;
use crate::service::Service;
use async_trait::async_trait;

pub struct StopCommand<'a> {
    pub service: &'a Service,
    pub service_name: &'a str,
}

#[async_trait]
impl<'a> ContainerCommand for StopCommand<'a> {
    async fn exec(&self, backend: &dyn ContainerBackend) -> Result<()> {
        let name = self.service.generate_name(self.service_name);
        backend.stop(&name, None).await
    }
}
