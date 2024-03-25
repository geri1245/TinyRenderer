use wgpu::{
    ColorTargetState, CommandEncoder, Device, Face, FragmentState, Operations,
    RenderPassColorAttachment, ShaderModule, TextureFormat,
};

use crate::{
    bind_group_layout_descriptors, buffer_content::BufferContent, model::RenderableMesh, vertex,
};

use super::{render_pipeline_base::PipelineBase, PipelineRecreationResult};

const SHADER_SOURCE: &'static str = "src/shaders/equirectangular_to_cubemap.wgsl";

pub struct EquirectangularToCubemapRP {
    render_pipeline: wgpu::RenderPipeline,
    shader_modification_time: u64,
    color_format: TextureFormat,
}

impl PipelineBase for EquirectangularToCubemapRP {}

impl EquirectangularToCubemapRP {
    pub async fn new(device: &wgpu::Device, color_format: TextureFormat) -> anyhow::Result<Self> {
        let shader = Self::compile_shader_if_needed(SHADER_SOURCE, device).await?;

        Ok(Self::new_internal(
            device,
            &shader.shader_module,
            shader.last_write_time,
            color_format,
        ))
    }

    fn new_internal(
        device: &wgpu::Device,
        shader: &ShaderModule,
        shader_compilation_time: u64,
        color_format: wgpu::TextureFormat,
    ) -> Self {
        let render_pipeline = {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("equirec to cubemap pipeline layout"),
                bind_group_layouts: &[
                    &device.create_bind_group_layout(
                        &(bind_group_layout_descriptors::BUFFER_VISIBLE_EVERYWHERE),
                    ),
                    &device.create_bind_group_layout(
                        &(bind_group_layout_descriptors::TEXTURE_2D_FRAGMENT_WITH_SAMPLER),
                    ),
                ],
                push_constant_ranges: &[],
            });

            // Create the render pipeline
            let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("equirec to cubemap render pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: shader,
                    entry_point: "vs_main",
                    buffers: &[vertex::VertexRawWithTangents::buffer_layout()],
                },
                fragment: Some(FragmentState {
                    module: shader,
                    entry_point: "fs_main",
                    targets: &[Some(ColorTargetState {
                        format: color_format,
                        blend: Some(wgpu::BlendState {
                            alpha: wgpu::BlendComponent::REPLACE,
                            color: wgpu::BlendComponent::REPLACE,
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(Face::Back),
                    unclipped_depth: device
                        .features()
                        .contains(wgpu::Features::DEPTH_CLIP_CONTROL),
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

            render_pipeline
        };

        Self {
            render_pipeline,
            shader_modification_time: shader_compilation_time,
            color_format,
        }
    }

    pub async fn try_recompile_shader(&self, device: &Device) -> PipelineRecreationResult<Self> {
        if !Self::need_recompile_shader(SHADER_SOURCE, self.shader_modification_time).await {
            return PipelineRecreationResult::AlreadyUpToDate;
        }

        match Self::compile_shader_if_needed(SHADER_SOURCE, device).await {
            Ok(compiled_shader) => PipelineRecreationResult::Success(Self::new_internal(
                device,
                &compiled_shader.shader_module,
                compiled_shader.last_write_time,
                self.color_format,
            )),
            Err(error) => PipelineRecreationResult::Failed(error),
        }
    }

    pub fn render(
        &self,
        encoder: &mut CommandEncoder,
        color_target: &wgpu::TextureView,
        renderable: &RenderableMesh,
        projection_bind_group: &wgpu::BindGroup,
        hdr_texture_bind_group: &wgpu::BindGroup,
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Equirec to cubemap pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: color_target,
                resolve_target: None,
                ops: Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 1.0,
                        g: 0.0,
                        b: 1.0,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, projection_bind_group, &[]);
        render_pass.set_bind_group(1, hdr_texture_bind_group, &[]);

        render_pass.set_vertex_buffer(0, renderable.vertex_buffer.slice(..));
        render_pass.set_index_buffer(renderable.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..renderable.index_count, 0, 0..1 as u32);
    }
}
