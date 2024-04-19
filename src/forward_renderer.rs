use wgpu::{BindGroup, Device, RenderPass};

use crate::{
    model::InstancedRenderableMesh,
    pipelines::{ForwardRP, ShaderCompilationSuccess},
};

pub struct ForwardRenderer {
    forward_rp: ForwardRP,
    texture_format: wgpu::TextureFormat,
}

impl ForwardRenderer {
    pub async fn new(device: &wgpu::Device, texture_format: wgpu::TextureFormat) -> Self {
        let pipeline = ForwardRP::new(device, texture_format).await.unwrap();

        Self {
            forward_rp: pipeline,
            texture_format,
        }
    }

    pub async fn try_recompile_shader(
        &mut self,
        device: &Device,
    ) -> anyhow::Result<ShaderCompilationSuccess> {
        self.forward_rp
            .try_recompile_shader(device, self.texture_format)
            .await
    }

    pub fn render<'a>(
        &'a self,
        render_pass: &mut RenderPass<'a>,
        mesh: &'a InstancedRenderableMesh,
        camera_bind_group: &'a BindGroup,
        light_bind_group: &'a BindGroup,
    ) {
        self.forward_rp
            .render_model(render_pass, mesh, camera_bind_group, light_bind_group);
    }
}
