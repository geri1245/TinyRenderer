use std::rc::Rc;

use wgpu::{RenderPass, TextureFormat};

use crate::{
    bind_group_layout_descriptors,
    camera_controller::CameraController,
    pipelines::{self, SkyboxRP},
    texture::SampledTexture,
};

pub struct Skybox {
    skybox_rp: SkyboxRP,
    _bind_group: Rc<wgpu::BindGroup>,
}

impl Skybox {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, texture_format: TextureFormat) -> Self {
        let skybox_rp = pipelines::SkyboxRP::new(device, texture_format);

        let texture = SampledTexture::create_skybox_texture(&device, &queue);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &device.create_bind_group_layout(
                &bind_group_layout_descriptors::TEXTURE_CUBE_FRAGMENT_WITH_SAMPLER,
            ),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&texture.sampler),
                },
            ],
            label: None,
        });

        Skybox {
            skybox_rp,
            _bind_group: Rc::new(bind_group),
        }
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
