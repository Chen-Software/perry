use crate::error::ComposeError;
use crate::commands::ContainerCommand;
use async_trait::async_trait;

pub struct RunCommand {
    pub service_name: String,
    pub service: std::sync::Arc<crate::service::Service>,
}

#[async_trait]
impl ContainerCommand for RunCommand {
    async fn exec(&self, backend: &dyn crate::backend::ContainerBackend) -> Result<(), ComposeError> {
        self.service.run_command(&self.service_name, backend).await.map(|_| ())
    }
}
