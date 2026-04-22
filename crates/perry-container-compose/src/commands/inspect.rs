use crate::backend::ContainerBackend;
use crate::commands::ContainerCommand;
use crate::error::Result;
use crate::service::Service;
use async_trait::async_trait;

pub struct InspectCommand<'a> {
    pub service: &'a Service,
    pub service_name: &'a str,
}

#[async_trait]
impl<'a> ContainerCommand for InspectCommand<'a> {
    async fn exec(&self, backend: &dyn ContainerBackend) -> Result<()> {
        self.service.inspect_command(self.service_name, backend).await.map(|_| ())
    }
}
