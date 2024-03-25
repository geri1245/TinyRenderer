use wgpu::{Device, RenderPass, TextureFormat};

use crate::{
    camera_controller::CameraController,
    pipelines::{self, SkyboxRP},
};

pub struct Skybox {
    skybox_rp: SkyboxRP,
    texture_format: TextureFormat,
}

impl Skybox {
    pub async fn new(device: &wgpu::Device, texture_format: TextureFormat) -> Self {
        let skybox_rp = pipelines::SkyboxRP::new(device, texture_format)
            .await
            .unwrap();

        Skybox {
            skybox_rp,
            texture_format,
        }
    }

    pub async fn try_recompile_shader(&mut self, device: &Device) -> anyhow::Result<()> {
        self.skybox_rp
            .try_recompile_shader(device, self.texture_format)
            .await
    }

    pub fn render<'a, 'b: 'a>(
        &'b self,
        render_pass: &mut RenderPass<'a>,
        camera_controller: &'b CameraController,
        cubemap_bind_group: &'b wgpu::BindGroup,
    ) {
        render_pass.push_debug_group("Skybox rendering");
        self.skybox_rp
            .render(render_pass, camera_controller, &cubemap_bind_group);
        render_pass.pop_debug_group();
    }
}
