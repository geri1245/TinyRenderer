use std::borrow::Cow;

use crate::{bind_group_layout_descriptors, camera_controller::CameraController, texture};

pub struct SkyboxRP {
    pipeline: wgpu::RenderPipeline,
}

impl SkyboxRP {
    pub fn new(device: &wgpu::Device, texture_format: wgpu::TextureFormat) -> Self {
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Skybox pipeline layout"),
            bind_group_layouts: &[
                &device.create_bind_group_layout(
                    &bind_group_layout_descriptors::TEXTURE_CUBE_FRAGMENT_WITH_SAMPLER,
                ),
                &device.create_bind_group_layout(
                    &bind_group_layout_descriptors::BUFFER_VISIBLE_EVERYWHERE,
                ),
            ],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Skybox shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("../shaders/skybox.wgsl"))),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Skybox pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_sky",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_sky",
                targets: &[Some(wgpu::ColorTargetState {
                    format: texture_format,
                    blend: Some(wgpu::BlendState {
                        alpha: wgpu::BlendComponent::REPLACE,
                        color: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                front_face: wgpu::FrontFace::Cw,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: texture::SampledTexture::DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        SkyboxRP { pipeline }
    }

    pub fn render<'a, 'b: 'a>(
        &'b self,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_controller: &'b CameraController,
        skybox_texture_bind_group: &'a wgpu::BindGroup,
    ) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(1, &camera_controller.bind_group, &[]);
        render_pass.set_bind_group(0, skybox_texture_bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
}
