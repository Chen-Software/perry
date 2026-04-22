use async_trait::async_trait;
use crate::commands::ContainerCommand;
use crate::error::Result;
use crate::backend::ContainerBackend;
use crate::types::ComposeServiceBuild;

pub struct BuildCommand<'a> {
    pub backend: &'a dyn ContainerBackend,
    pub spec: &'a ComposeServiceBuild,
    pub image_name: String,
}

#[async_trait]
impl<'a> ContainerCommand for BuildCommand<'a> {
    async fn exec(&self) -> Result<()> {
        self.backend.build(self.spec, &self.image_name).await
    }
}
