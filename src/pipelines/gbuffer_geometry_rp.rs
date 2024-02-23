use wgpu::{
    BindGroup, Buffer, CommandEncoder, RenderPass, RenderPassColorAttachment,
    RenderPassDepthStencilAttachment, RenderPipeline, TextureFormat,
};

use crate::{
    bind_group_layout_descriptors,
    buffer_content::BufferContent,
    instance,
    model::{Mesh, Model},
    texture::{self, SampledTexture},
    vertex,
};

const CLEAR_COLOR: wgpu::Color = wgpu::Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.0,
};

pub struct GBufferTextures {
    pub position: SampledTexture,
    pub normal: SampledTexture,
    pub albedo_and_specular: SampledTexture,
    pub depth_texture: SampledTexture,
    pub metal_rough_ao: SampledTexture,
}

pub struct GBufferGeometryRP {
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

impl GBufferGeometryRP {
    fn create_pipeline(device: &wgpu::Device, textures: &GBufferTextures) -> RenderPipeline {
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Geometry pass pipeline layout"),
            bind_group_layouts: &[
                &device.create_bind_group_layout(&bind_group_layout_descriptors::PBR_TEXTURE),
                &device.create_bind_group_layout(&bind_group_layout_descriptors::CAMERA),
            ],
            push_constant_ranges: &[],
        });

        let gbuffer_shader_desc = wgpu::ShaderModuleDescriptor {
            label: Some("Geometry pass shader desc"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/gbuffer_geometry.wgsl").into(),
            ),
        };

        let gbuffer_shader = device.create_shader_module(gbuffer_shader_desc);

        let gbuffer_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gbuffer pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &gbuffer_shader,
                entry_point: "vs_main",
                buffers: &[
                    vertex::VertexRawWithTangents::buffer_layout(),
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
                    default_color_write_state(textures.metal_rough_ao.format),
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
                format: texture::SampledTexture::DEPTH_FORMAT,
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
        let position_texture = SampledTexture::new(
            device,
            TextureFormat::Rgba16Float,
            width,
            height,
            "GBuffer position texture",
        );
        let normal_texture = SampledTexture::new(
            device,
            TextureFormat::Rgba16Float,
            width,
            height,
            "GBuffer normal texture",
        );
        let albedo_and_specular_texture = SampledTexture::new(
            device,
            TextureFormat::Rgba8Unorm,
            width,
            height,
            "GBuffer albedo texture",
        );
        let metal_rough_ao = SampledTexture::new(
            device,
            TextureFormat::Rgba16Float,
            width,
            height,
            "GBuffer albedo texture",
        );

        let depth_texture_extents = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let depth_texture = SampledTexture::create_depth_texture(
            device,
            depth_texture_extents,
            "GBuffer depth texture",
        );

        GBufferTextures {
            position: position_texture,
            normal: normal_texture,
            albedo_and_specular: albedo_and_specular_texture,
            depth_texture,
            metal_rough_ao,
        }
    }

    fn create_bind_group(device: &wgpu::Device, textures: &GBufferTextures) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &device.create_bind_group_layout(&bind_group_layout_descriptors::GBUFFER),
            entries: &[
                textures.position.get_texture_bind_group_entry(0),
                textures.position.get_sampler_bind_group_entry(1),
                textures.normal.get_texture_bind_group_entry(2),
                textures.normal.get_sampler_bind_group_entry(3),
                textures.albedo_and_specular.get_texture_bind_group_entry(4),
                textures.albedo_and_specular.get_sampler_bind_group_entry(5),
                textures.metal_rough_ao.get_texture_bind_group_entry(6),
                textures.metal_rough_ao.get_sampler_bind_group_entry(7),
            ],
            label: Some("GBuffer bind group"),
        })
    }

    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let textures = Self::create_textures(device, width, height);
        let pipeline = Self::create_pipeline(device, &textures);
        let bind_group = Self::create_bind_group(device, &textures);

        Self {
            textures,
            render_pipeline: pipeline,
            bind_group,
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.textures = Self::create_textures(device, width, height);
        self.bind_group = Self::create_bind_group(device, &self.textures);
    }

    pub fn render_model<'a>(
        &'a self,
        render_pass: &mut RenderPass<'a>,
        model: &'a Model,
        camera_bind_group: &'a BindGroup,
        instances: usize,
        instance_buffer: &'a Buffer,
    ) {
        self.prepare_render(render_pass, camera_bind_group, instance_buffer);
        for mesh in &model.meshes {
            self.render_mesh_internal(render_pass, mesh, instances);
        }
    }

    pub fn render_mesh<'a>(
        &'a self,
        render_pass: &mut RenderPass<'a>,
        mesh: &'a Mesh,
        camera_bind_group: &'a BindGroup,
        instances: usize,
        instance_buffer: &'a Buffer,
    ) {
        self.prepare_render(render_pass, camera_bind_group, instance_buffer);
        self.render_mesh_internal(render_pass, mesh, instances);
    }

    pub fn begin_render<'a>(&'a self, encoder: &'a mut CommandEncoder) -> RenderPass<'a> {
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("GBuffer pass"),
            color_attachments: &[
                Some(RenderPassColorAttachment {
                    view: &self.textures.position.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(CLEAR_COLOR),
                        store: wgpu::StoreOp::Store,
                    },
                }),
                Some(RenderPassColorAttachment {
                    view: &self.textures.normal.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(CLEAR_COLOR),
                        store: wgpu::StoreOp::Store,
                    },
                }),
                Some(RenderPassColorAttachment {
                    view: &self.textures.albedo_and_specular.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(CLEAR_COLOR),
                        store: wgpu::StoreOp::Store,
                    },
                }),
                Some(RenderPassColorAttachment {
                    view: &self.textures.metal_rough_ao.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(CLEAR_COLOR),
                        store: wgpu::StoreOp::Store,
                    },
                }),
            ],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: &self.textures.depth_texture.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        })
    }

    fn render_mesh_internal<'a>(
        &self,
        render_pass: &mut RenderPass<'a>,
        mesh: &'a Mesh,
        instances: usize,
    ) {
        render_pass.set_bind_group(0, &mesh.material.bind_group, &[]);
        render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..mesh.index_count, 0, 0..instances as u32);
    }

    fn prepare_render<'a>(
        &'a self,
        render_pass: &mut RenderPass<'a>,
        camera_bind_group: &'a BindGroup,
        instance_buffer: &'a Buffer,
    ) {
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(1, &camera_bind_group, &[]);
        render_pass.set_vertex_buffer(1, instance_buffer.slice(..));
    }
}
