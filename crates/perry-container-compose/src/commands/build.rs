use crate::error::ComposeError;
use crate::commands::ContainerCommand;
use async_trait::async_trait;

pub struct BuildCommand {
    pub service_name: String,
    pub service: std::sync::Arc<crate::service::Service>,
}

#[async_trait]
impl ContainerCommand for BuildCommand {
    async fn exec(&self, backend: &dyn crate::backend::ContainerBackend) -> Result<(), ComposeError> {
        self.service.build_command(&self.service_name, backend).await
    }
}
