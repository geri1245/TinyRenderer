use std::collections::HashMap;

use wgpu::{
    BindGroup, Buffer, RenderPassColorAttachment, RenderPassDepthStencilAttachment, TextureFormat,
};

use crate::{
    buffer_content::BufferContent,
    instance,
    model::Model,
    renderer::BindGroupLayoutType,
    texture::{self, Texture},
    vertex,
};

pub struct GBuffer {
    pub position_texture: Texture,
    pub normal_texture: Texture,
    pub albedo_and_specular_texture: Texture,
    pub depth_texture: Texture,
    pub render_pipeline: wgpu::RenderPipeline,
    pub bind_group: wgpu::BindGroup,
}

fn default_color_write_state(format: wgpu::TextureFormat) -> Option<wgpu::ColorTargetState> {
    Some(wgpu::ColorTargetState {
        format,
        blend: Some(wgpu::BlendState {
            alpha: wgpu::BlendComponent::REPLACE,
            color: wgpu::BlendComponent::REPLACE,
        }),
        write_mask: wgpu::ColorWrites::ALL,
    })
}

impl GBuffer {
    pub fn new(
        device: &wgpu::Device,
        bind_group_layouts: &HashMap<BindGroupLayoutType, wgpu::BindGroupLayout>,
        width: u32,
        height: u32,
    ) -> Self {
        let position_texture = Texture::new(
            device,
            TextureFormat::Rgba16Float,
            width,
            height,
            "GBuffer position texture",
        );
        let normal_texture = Texture::new(
            device,
            TextureFormat::Rgba16Float,
            width,
            height,
            "GBuffer normal texture",
        );
        let albedo_and_specular_texture = Texture::new(
            device,
            TextureFormat::Rgba8Unorm,
            width,
            height,
            "GBuffer albedo texture",
        );

        let depth_texture =
            Texture::create_depth_texture(device, width, height, "GBuffer depth texture");

        let gbuffer_pipeline = {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Fill gbuffer pipeline layout"),
                bind_group_layouts: &[
                    &bind_group_layouts
                        .get(&BindGroupLayoutType::DiffuseTexture)
                        .unwrap(),
                    &bind_group_layouts
                        .get(&BindGroupLayoutType::Camera)
                        .unwrap(),
                ],
                push_constant_ranges: &[],
            });

            let gbuffer_shader_desc = wgpu::ShaderModuleDescriptor {
                label: Some("Fill gbuffer shader desc"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/fill_gbuffer.wgsl").into()),
            };

            let gbuffer_shader = device.create_shader_module(gbuffer_shader_desc);

            let gbuffer_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("gbuffer pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &gbuffer_shader,
                    entry_point: "vs_main",
                    buffers: &[
                        vertex::VertexRaw::buffer_layout(),
                        instance::InstanceRaw::buffer_layout(),
                    ],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &gbuffer_shader,
                    entry_point: "fs_main",
                    targets: &[
                        default_color_write_state(position_texture.format),
                        default_color_write_state(normal_texture.format),
                        default_color_write_state(albedo_and_specular_texture.format),
                    ],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    unclipped_depth: device
                        .features()
                        .contains(wgpu::Features::DEPTH_CLIP_CONTROL),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: texture::Texture::DEPTH_FORMAT,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

            gbuffer_pipeline
        };

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: bind_group_layouts
                .get(&BindGroupLayoutType::GBuffer)
                .unwrap(),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&position_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&position_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&normal_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&normal_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&albedo_and_specular_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&albedo_and_specular_texture.sampler),
                },
            ],
            label: Some("GBuffer bind group"),
        });

        GBuffer {
            position_texture,
            normal_texture,
            albedo_and_specular_texture,
            render_pipeline: gbuffer_pipeline,
            depth_texture,
            bind_group,
        }
    }

    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        model: &Model,
        camera_bind_group: &BindGroup,
        instances: usize,
        instance_buffer: &Buffer,
    ) {
        let mut gbuffer_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("GBuffer pass"),
            color_attachments: &[
                Some(RenderPassColorAttachment {
                    view: &self.position_texture.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        }),
                        store: true,
                    },
                }),
                Some(RenderPassColorAttachment {
                    view: &self.normal_texture.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        }),
                        store: true,
                    },
                }),
                Some(RenderPassColorAttachment {
                    view: &self.albedo_and_specular_texture.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        }),
                        store: true,
                    },
                }),
            ],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: &self.depth_texture.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: true,
                }),
                stencil_ops: None,
            }),
        });

        gbuffer_pass.set_pipeline(&self.render_pipeline);

        gbuffer_pass.set_bind_group(1, &camera_bind_group, &[]);

        gbuffer_pass.set_vertex_buffer(1, instance_buffer.slice(..));

        for mesh in &model.meshes {
            gbuffer_pass.set_bind_group(0, &mesh.material.as_ref().unwrap().bind_group, &[]);
            gbuffer_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            gbuffer_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            gbuffer_pass.draw_indexed(0..mesh.index_count, 0, 0..instances as u32);
        }
    }
}
