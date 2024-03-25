use wgpu::{
    BindGroup, CommandEncoder, Device, RenderPassDepthStencilAttachment, ShaderModule,
    TextureFormat,
};

use crate::{
    bind_group_layout_descriptors::{self},
    buffer_content::BufferContent,
    instance, vertex,
    world_renderer::MeshType,
};

use super::{render_pipeline_base::PipelineBase, PipelineRecreationResult};

const SHADER_SOURCE: &'static str = "src/shaders/shadow.wgsl";
// TODO: share this with the shadow code, don't define this again
const DEPTH_FORMAT: TextureFormat = TextureFormat::Depth32Float;

// TODO: Can we get away with not using RCs here? If we don't have RCs, then when we
// are recreating the shader and thus the pipeline, then we can't move out of these fields.
// However semantically this is just giving away the ownership to the new pipeline,
// but Rust doesn't know that. I should try to tell it somehow...
pub struct ShadowRP {
    shadow_pipeline: wgpu::RenderPipeline,
    shader_modification_time: u64,
}

impl PipelineBase for ShadowRP {}

impl ShadowRP {
    pub async fn new(device: &wgpu::Device) -> anyhow::Result<ShadowRP> {
        let shader = Self::compile_shader_if_needed(SHADER_SOURCE, device).await?;

        Ok(Self::new_internal(
            device,
            &shader.shader_module,
            shader.last_write_time,
        ))
    }

    fn new_internal(
        device: &wgpu::Device,
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
                        instance::SceneComponentRaw::buffer_layout(),
                    ],
                },
                fragment: None,
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    // IMPORTANT: The face culling is set to Back faces here, but because there is a negative multiplier
                    // in the shader, this will actually mean front face culling (so we are actually drawing back faces here)
                    cull_mode: Some(wgpu::Face::Back),
                    unclipped_depth: device
                        .features()
                        .contains(wgpu::Features::DEPTH_CLIP_CONTROL),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
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
            shadow_pipeline,
            shader_modification_time: shader_compilation_time,
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
            )),
            Err(error) => PipelineRecreationResult::Failed(error),
        }
    }

    pub fn render(
        &self,
        encoder: &mut CommandEncoder,
        meshes: &Vec<MeshType>,
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

        for mesh in meshes {
            if let MeshType::TexturedMesh(renderable) = mesh {
                let renderable = &renderable.mesh;
                shadow_pass.set_vertex_buffer(1, renderable.instance_buffer.slice(..));

                shadow_pass.set_vertex_buffer(0, renderable.mesh.vertex_buffer.slice(..));
                shadow_pass.set_index_buffer(
                    renderable.mesh.index_buffer.slice(..),
                    wgpu::IndexFormat::Uint32,
                );
                shadow_pass.draw_indexed(
                    0..renderable.mesh.index_count,
                    0,
                    0..renderable.instances.len() as u32,
                );
            }
        }
    }
}
