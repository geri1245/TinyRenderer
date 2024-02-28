use std::rc::Rc;

use wgpu::{BindGroup, CommandEncoder, Device, RenderPassDepthStencilAttachment, ShaderModule};

use crate::{
    bind_group_layout_descriptors, buffer_content::BufferContent, instance,
    model::InstancedRenderableMesh, texture::SampledTexture, vertex,
};

use super::{render_pipeline_base::PipelineBase, PipelineRecreationResult};

const SHADER_SOURCE: &'static str = "src/shaders/shadow.wgsl";

// TODO: Can we get away with not using RCs here? If we don't have RCs, then when we
// are recreating the shader and thus the pipeline, then we can't move out of these fields.
// However semantically this is just giving away the ownership to the new pipeline,
// but Rust doesn't know that. I should try to tell it somehow...
pub struct ShadowRP {
    shadow_pipeline: wgpu::RenderPipeline,
    pub bind_group: Rc<wgpu::BindGroup>,
    directional_shadow_texture: Rc<SampledTexture>,
    point_shadow_texture: Rc<SampledTexture>,
    shader_modification_time: u64,
}

impl PipelineBase for ShadowRP {}

impl ShadowRP {
    pub async fn new(
        device: &wgpu::Device,
        directional_shadow_texture: SampledTexture,
        point_shadow_texture: SampledTexture,
        directional_shadow_texture_view: wgpu::TextureView,
        point_shadow_texture_view: wgpu::TextureView,
    ) -> anyhow::Result<ShadowRP> {
        let shader = Self::compile_shader_if_needed(SHADER_SOURCE, device).await?;
        let bind_group = Self::create_bind_group(
            device,
            &directional_shadow_texture,
            &point_shadow_texture,
            directional_shadow_texture_view,
            point_shadow_texture_view,
        );

        Ok(Self::new_internal(
            device,
            Rc::new(directional_shadow_texture),
            Rc::new(point_shadow_texture),
            Rc::new(bind_group),
            &shader.shader_module,
            shader.last_write_time,
        ))
    }

    fn new_internal(
        device: &wgpu::Device,
        directional_shadow_texture: Rc<SampledTexture>,
        point_shadow_texture: Rc<SampledTexture>,
        bind_group: Rc<wgpu::BindGroup>,
        shader: &ShaderModule,
        shader_compilation_time: u64,
    ) -> Self {
        let shadow_pipeline = {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("shadow pipeline layout"),
                bind_group_layouts: &[&device.create_bind_group_layout(
                    &(bind_group_layout_descriptors::LIGHT_WITH_DYNAMIC_OFFSET),
                )],
                push_constant_ranges: &[],
            });

            // Create the render pipeline
            let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("shadow render pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: shader,
                    entry_point: "vs_main",
                    buffers: &[
                        vertex::VertexRawWithTangents::buffer_layout(),
                        instance::InstanceRaw::buffer_layout(),
                    ],
                },
                fragment: None,
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
                    format: directional_shadow_texture.texture.format(),
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState {
                        constant: 0,
                        slope_scale: 0.0,
                        clamp: 0.0,
                    },
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

            shadow_pipeline
        };

        ShadowRP {
            bind_group,
            shadow_pipeline,
            directional_shadow_texture,
            point_shadow_texture,
            shader_modification_time: shader_compilation_time,
        }
    }

    fn create_bind_group(
        device: &wgpu::Device,
        directional_shadow_texture: &SampledTexture,
        point_shadow_texture: &SampledTexture,
        directional_shadow_texture_view: wgpu::TextureView,
        point_shadow_texture_view: wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &device.create_bind_group_layout(&bind_group_layout_descriptors::DEPTH_TEXTURE),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&directional_shadow_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&directional_shadow_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&point_shadow_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&point_shadow_texture.sampler),
                },
            ],
            label: None,
        })
    }

    pub async fn try_recompile_shader(&self, device: &Device) -> PipelineRecreationResult<Self> {
        if !Self::need_recompile_shader(SHADER_SOURCE, self.shader_modification_time).await {
            return PipelineRecreationResult::AlreadyUpToDate;
        }

        match Self::compile_shader_if_needed(SHADER_SOURCE, device).await {
            Ok(compiled_shader) => PipelineRecreationResult::Success(Self::new_internal(
                device,
                self.point_shadow_texture.clone(),
                self.directional_shadow_texture.clone(),
                self.bind_group.clone(),
                &compiled_shader.shader_module,
                compiled_shader.last_write_time,
            )),
            Err(error) => PipelineRecreationResult::Failed(error),
        }
    }

    pub fn render(
        &self,
        encoder: &mut CommandEncoder,
        mesh: &InstancedRenderableMesh,
        light_bind_group: &BindGroup,
        depth_target: &wgpu::TextureView,
        light_bind_group_offset: u32,
    ) {
        let mut shadow_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Shadow pass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: depth_target,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        shadow_pass.set_pipeline(&self.shadow_pipeline);

        shadow_pass.set_bind_group(0, &light_bind_group, &[light_bind_group_offset]);

        shadow_pass.set_vertex_buffer(1, mesh.instance_buffer.slice(..));

        shadow_pass.set_vertex_buffer(0, mesh.mesh.mesh.vertex_buffer.slice(..));
        shadow_pass.set_index_buffer(
            mesh.mesh.mesh.index_buffer.slice(..),
            wgpu::IndexFormat::Uint32,
        );
        shadow_pass.draw_indexed(
            0..mesh.mesh.mesh.index_count,
            0,
            0..mesh.instances.len() as u32,
        );
    }
}
