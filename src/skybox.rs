use wgpu::RenderPass;

use crate::{
    camera_controller::CameraController,
    pipelines::{self, SkyboxRP},
    renderer::Renderer,
};

pub struct Skybox {
    skybox_rp: SkyboxRP,
}

impl Skybox {
    pub fn new(renderer: &Renderer) -> Self {
        let skybox_rp =
            pipelines::SkyboxRP::new(&renderer.device, &renderer.queue, renderer.config.format);

        Skybox { skybox_rp }
    }

    pub fn render<'a, 'b: 'a>(
        &'b self,
        render_pass: &mut RenderPass<'a>,
        camera_controller: &'b CameraController,
    ) {
        render_pass.push_debug_group("Skybox rendering");
        self.skybox_rp.render(render_pass, camera_controller);
        render_pass.pop_debug_group();
    }
}
