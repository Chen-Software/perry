use crate::error::ComposeError;
use crate::commands::ContainerCommand;
use async_trait::async_trait;

pub struct StopCommand {
    pub service_name: String,
    pub service: std::sync::Arc<crate::service::Service>,
}

#[async_trait]
impl ContainerCommand for StopCommand {
    async fn exec(&self, backend: &dyn crate::backend::ContainerBackend) -> Result<(), ComposeError> {
        let name = self.service.generate_name(&self.service_name);
        backend.stop(&name, None).await
    }
}
