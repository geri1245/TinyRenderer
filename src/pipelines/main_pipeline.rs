use crate::{
    bind_group_layout_descriptors, camera_controller::CameraController,
    light_controller::LightController, texture,
};

use super::render_pipeline;

pub struct MainPass {
    render_pipeline: wgpu::RenderPipeline,
}

impl MainPass {
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        let shader_desc = wgpu::ShaderModuleDescriptor {
            label: Some("GBuffer processing shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/gbuffer_pass.wgsl").into()),
        };

        let main_shader = device.create_shader_module(shader_desc);

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Main Render Pipeline Layout"),
                bind_group_layouts: &[
                    &device.create_bind_group_layout(&bind_group_layout_descriptors::LIGHT),
                    &device.create_bind_group_layout(&bind_group_layout_descriptors::CAMERA),
                    &device.create_bind_group_layout(&bind_group_layout_descriptors::GBUFFER),
                    &device.create_bind_group_layout(&bind_group_layout_descriptors::DEPTH_TEXTURE),
                ],
                push_constant_ranges: &[],
            });

        let render_pipeline = render_pipeline::new(
            Some("Main render pipeline"),
            &device,
            &render_pipeline_layout,
            color_format,
            Some(texture::Texture::DEPTH_FORMAT),
            &[],
            &main_shader,
        );

        Self { render_pipeline }
    }

    pub fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_controller: &'a CameraController,
        light_controller: &'a LightController,
        gbuffer_bind_group: &'a wgpu::BindGroup,
        shadow_bind_group: &'a wgpu::BindGroup,
    ) {
        render_pass.push_debug_group("Cubes rendering from GBuffer");

        render_pass.set_pipeline(&self.render_pipeline);

        render_pass.set_bind_group(1, &camera_controller.bind_group, &[]);
        render_pass.set_bind_group(0, &light_controller.bind_group, &[]);
        render_pass.set_bind_group(2, gbuffer_bind_group, &[]);
        render_pass.set_bind_group(3, shadow_bind_group, &[]);

        render_pass.draw(0..3, 0..1);

        render_pass.pop_debug_group();
    }
}
