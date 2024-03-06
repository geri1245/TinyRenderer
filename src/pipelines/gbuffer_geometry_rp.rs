use wgpu::{
    BindGroup, CommandEncoder, Device, RenderPass, RenderPassColorAttachment,
    RenderPassDepthStencilAttachment, RenderPipeline, ShaderModule, TextureFormat, TextureUsages,
};

use crate::{
    bind_group_layout_descriptors,
    buffer_content::BufferContent,
    instance,
    model::InstancedRenderableMesh,
    texture::{self, SampledTexture, SampledTextureDescriptor},
    vertex,
};

use super::{
    render_pipeline_base::PipelineBase, shader_compilation_result::CompiledShader,
    PipelineRecreationResult,
};

const GBUFFER_TEXTURE_FORMAT: TextureFormat = TextureFormat::Rgba16Float;

const GBUFFER_CLEAR_COLOR: wgpu::Color = wgpu::Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.0,
};

const SHADER_SOURCE: &'static str = "src/shaders/gbuffer_geometry.wgsl";

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
    shader_modification_time: u64,
    width: u32,
    height: u32,
}

impl PipelineBase for GBufferGeometryRP {}

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
    fn create_pipeline(
        device: &wgpu::Device,
        shader: &ShaderModule,
        textures: &GBufferTextures,
    ) -> RenderPipeline {
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Geometry pass pipeline layout"),
            bind_group_layouts: &[
                &device.create_bind_group_layout(&bind_group_layout_descriptors::PBR_TEXTURE),
                &device.create_bind_group_layout(&bind_group_layout_descriptors::CAMERA),
            ],
            push_constant_ranges: &[],
        });

        let gbuffer_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gbuffer pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: "vs_main",
                buffers: &[
                    vertex::VertexRawWithTangents::buffer_layout(),
                    instance::InstanceRaw::buffer_layout(),
                ],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[
                    default_color_write_state(textures.position.texture.format()),
                    default_color_write_state(textures.normal.texture.format()),
                    default_color_write_state(textures.albedo_and_specular.texture.format()),
                    default_color_write_state(textures.metal_rough_ao.texture.format()),
                ],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                unclipped_depth: false,
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
        let descriptor = SampledTextureDescriptor {
            width,
            height,
            format: GBUFFER_TEXTURE_FORMAT,
            usages: TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING,
        };

        let position_texture = SampledTexture::new(device, &descriptor, "GBuffer position texture");
        let normal_texture = SampledTexture::new(device, &descriptor, "GBuffer normal texture");
        let albedo_and_specular_texture =
            SampledTexture::new(device, &descriptor, "GBuffer albedo and specular texture");
        let metal_rough_ao =
            SampledTexture::new(device, &descriptor, "GBuffer metal+rough+ao texture");

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

    fn new_internal(
        shader: &CompiledShader,
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> Self {
        let textures = Self::create_textures(device, width, height);
        let pipeline = Self::create_pipeline(device, &shader.shader_module, &textures);
        let bind_group = Self::create_bind_group(device, &textures);

        Self {
            textures,
            render_pipeline: pipeline,
            bind_group,
            shader_modification_time: shader.last_write_time,
            width,
            height,
        }
    }

    pub async fn new(device: &wgpu::Device, width: u32, height: u32) -> anyhow::Result<Self> {
        let shader = Self::compile_shader_if_needed(SHADER_SOURCE, device).await?;
        Ok(Self::new_internal(&shader, device, width, height))
    }

    pub async fn try_recompile_shader(&self, device: &Device) -> PipelineRecreationResult<Self> {
        if !Self::need_recompile_shader(SHADER_SOURCE, self.shader_modification_time).await {
            return PipelineRecreationResult::AlreadyUpToDate;
        }

        match Self::compile_shader_if_needed(SHADER_SOURCE, device).await {
            Ok(compiled_shader) => PipelineRecreationResult::Success(Self::new_internal(
                &compiled_shader,
                device,
                self.width,
                self.height,
            )),
            Err(error) => PipelineRecreationResult::Failed(error),
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.textures = Self::create_textures(device, width, height);
        self.bind_group = Self::create_bind_group(device, &self.textures);
        self.width = width;
        self.height = height;
    }

    pub fn render_mesh<'a>(
        &'a self,
        render_pass: &mut RenderPass<'a>,
        mesh: &'a InstancedRenderableMesh,
        camera_bind_group: &'a BindGroup,
    ) {
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(1, &camera_bind_group, &[]);
        render_pass.set_vertex_buffer(1, mesh.instance_buffer.slice(..));

        render_pass.set_bind_group(0, &mesh.mesh.material.bind_group, &[]);
        render_pass.set_vertex_buffer(0, mesh.mesh.mesh.vertex_buffer.slice(..));
        render_pass.set_index_buffer(
            mesh.mesh.mesh.index_buffer.slice(..),
            wgpu::IndexFormat::Uint32,
        );
        render_pass.draw_indexed(
            0..mesh.mesh.mesh.index_count,
            0,
            0..mesh.instances.len() as u32,
        );
    }

    pub fn begin_render<'a>(&'a self, encoder: &'a mut CommandEncoder) -> RenderPass<'a> {
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("GBuffer pass"),
            color_attachments: &[
                Some(RenderPassColorAttachment {
                    view: &self.textures.position.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(GBUFFER_CLEAR_COLOR),
                        store: wgpu::StoreOp::Store,
                    },
                }),
                Some(RenderPassColorAttachment {
                    view: &self.textures.normal.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(GBUFFER_CLEAR_COLOR),
                        store: wgpu::StoreOp::Store,
                    },
                }),
                Some(RenderPassColorAttachment {
                    view: &self.textures.albedo_and_specular.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(GBUFFER_CLEAR_COLOR),
                        store: wgpu::StoreOp::Store,
                    },
                }),
                Some(RenderPassColorAttachment {
                    view: &self.textures.metal_rough_ao.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(GBUFFER_CLEAR_COLOR),
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
}
