use std::collections::HashMap;

use wgpu::{
    BindGroup, Buffer, RenderPassColorAttachment, RenderPassDepthStencilAttachment, RenderPipeline,
    TextureFormat,
};

use crate::{
    buffer_content::BufferContent,
    instance,
    model::Model,
    renderer::BindGroupLayoutType,
    texture::{self, Texture},
    vertex,
};

pub struct GBufferTextures {
    pub position: Texture,
    pub normal: Texture,
    pub albedo_and_specular: Texture,
    pub depth_texture: Texture,
}

pub struct GBuffer {
    pub textures: GBufferTextures,
    render_pipeline: wgpu::RenderPipeline,
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
    fn create_pipeline(
        device: &wgpu::Device,
        bind_group_layouts: &HashMap<BindGroupLayoutType, wgpu::BindGroupLayout>,
        textures: &GBufferTextures,
    ) -> RenderPipeline {
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
                    default_color_write_state(textures.position.format),
                    default_color_write_state(textures.normal.format),
                    default_color_write_state(textures.albedo_and_specular.format),
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
    }

    pub fn create_textures(device: &wgpu::Device, width: u32, height: u32) -> GBufferTextures {
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

        GBufferTextures {
            position: position_texture,
            normal: normal_texture,
            albedo_and_specular: albedo_and_specular_texture,
            depth_texture,
        }
    }

    fn create_bind_group(
        device: &wgpu::Device,
        bind_group_layouts: &HashMap<BindGroupLayoutType, wgpu::BindGroupLayout>,
        textures: &GBufferTextures,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: bind_group_layouts
                .get(&BindGroupLayoutType::GBuffer)
                .unwrap(),
            entries: &[
                textures.position.get_texture_bind_group_entry(0),
                textures.position.get_sampler_bind_group_entry(1),
                textures.normal.get_texture_bind_group_entry(2),
                textures.normal.get_sampler_bind_group_entry(3),
                textures.albedo_and_specular.get_texture_bind_group_entry(4),
                textures.albedo_and_specular.get_sampler_bind_group_entry(5),
            ],
            label: Some("GBuffer bind group"),
        })
    }

    pub fn new(
        device: &wgpu::Device,
        bind_group_layouts: &HashMap<BindGroupLayoutType, wgpu::BindGroupLayout>,
        width: u32,
        height: u32,
    ) -> Self {
        let textures = Self::create_textures(device, width, height);
        let pipeline = Self::create_pipeline(device, bind_group_layouts, &textures);
        let bind_group = Self::create_bind_group(device, bind_group_layouts, &textures);

        GBuffer {
            textures,
            render_pipeline: pipeline,
            bind_group,
        }
    }

    pub fn resize(
        &mut self,
        device: &wgpu::Device,
        bind_group_layouts: &HashMap<BindGroupLayoutType, wgpu::BindGroupLayout>,
        width: u32,
        height: u32,
    ) {
        self.textures = Self::create_textures(device, width, height);
        self.bind_group = Self::create_bind_group(device, bind_group_layouts, &self.textures);
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
                    view: &self.textures.position.view,
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
                    view: &self.textures.normal.view,
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
                    view: &self.textures.albedo_and_specular.view,
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
                view: &self.textures.depth_texture.view,
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
