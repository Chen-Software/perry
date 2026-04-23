use crate::error::Result;
use crate::backend::ContainerBackend;
use crate::error::ComposeError;
use crate::types::ComposeService;

#[async_trait::async_trait]
pub trait ContainerCommand: Send + Sync {
    async fn exec(&self, backend: &dyn ContainerBackend) -> Result<()>;
}

pub struct BuildCommand<'a> {
    pub service_name: &'a str,
    pub service: &'a ComposeService,
}

#[async_trait::async_trait]
impl<'a> ContainerCommand for BuildCommand<'a> {
    async fn exec(&self, backend: &dyn ContainerBackend) -> Result<()> {
        if let Some(build) = &self.service.build {
            let build_config = build.as_build();
            backend.build_image(
                build_config.context.as_deref().unwrap_or("."),
                &self.service.image_ref(self.service_name),
                build_config.dockerfile.as_deref(),
                build_config.args.as_ref().map(|l| l.to_map()).as_ref(),
            ).await?;
        }
        Ok(())
    }
}

pub struct InspectCommand {
    pub container_id: String,
}

#[async_trait::async_trait]
impl ContainerCommand for InspectCommand {
    async fn exec(&self, backend: &dyn ContainerBackend) -> Result<()> {
        let _ = backend.inspect(&self.container_id).await?;
        Ok(())
    }
}

pub struct RunCommand<'a> {
    pub service_name: &'a str,
    pub service: &'a ComposeService,
}

#[async_trait::async_trait]
impl<'a> ContainerCommand for RunCommand<'a> {
    async fn exec(&self, backend: &dyn ContainerBackend) -> Result<()> {
        let spec = crate::types::ContainerSpec {
            image: self.service.image_ref(self.service_name),
            name: Some(crate::service::generate_name(self.service_name, self.service)?),
            ports: Some(self.service.port_strings()),
            volumes: Some(self.service.volume_strings()),
            env: Some(self.service.resolved_env()),
            cmd: self.service.command_list(),
            ..Default::default()
        };
        backend.run(&spec).await?;
        Ok(())
    }
}

pub struct StartCommand {
    pub container_id: String,
}

#[async_trait::async_trait]
impl ContainerCommand for StartCommand {
    async fn exec(&self, backend: &dyn ContainerBackend) -> Result<()> {
        backend.start(&self.container_id).await?;
        Ok(())
    }
}

pub struct StopCommand {
    pub container_id: String,
}

#[async_trait::async_trait]
impl ContainerCommand for StopCommand {
    async fn exec(&self, backend: &dyn ContainerBackend) -> Result<()> {
        backend.stop(&self.container_id, None).await?;
        Ok(())
    }
}
