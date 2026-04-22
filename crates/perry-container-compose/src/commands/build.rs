use async_trait::async_trait;
use crate::error::Result;
use crate::backend::ContainerBackend;
use crate::types::ComposeServiceBuild;
use super::ContainerCommand;

pub struct BuildCommand {
    pub spec: ComposeServiceBuild,
    pub image_name: String,
}

#[async_trait]
impl ContainerCommand for BuildCommand {
    async fn exec(&self, backend: &dyn ContainerBackend) -> Result<()> {
        backend.build(&self.spec, &self.image_name).await
    }
}
