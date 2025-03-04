use wgpu::{
    ColorTargetState, CommandEncoder, Device, Face, FragmentState, Operations,
    PipelineCompilationOptions, RenderPassColorAttachment, RenderPipeline, ShaderModule,
    TextureFormat,
};

use crate::{
    bind_group_layout_descriptors, buffer_content::BufferContent, model::Primitive, vertex,
};

use super::shader_compiler::{ShaderCompilationResult, ShaderCompilationSuccess, ShaderCompiler};

const SHADER_SOURCE: &'static str = "src/shaders/diffuse_irradiance_bake.wgsl";

pub struct DiffuseIrradianceBakerRP {
    render_pipeline: wgpu::RenderPipeline,
    shader_compiler: ShaderCompiler,
}

impl DiffuseIrradianceBakerRP {
    pub fn new(device: &wgpu::Device, color_format: TextureFormat) -> anyhow::Result<Self> {
        let mut shader_compiler = ShaderCompiler::new(SHADER_SOURCE.to_string());
        let shader_compilation_result = shader_compiler.compile_shader_if_needed(device)?;

        match shader_compilation_result {
            ShaderCompilationResult::AlreadyUpToDate => {
                panic!("This shader hasn't been compiled yet, can't be up to date!")
            }
            ShaderCompilationResult::Success(shader) => Ok(Self {
                render_pipeline: Self::create_pipeline(device, &shader, color_format),
                shader_compiler,
            }),
        }
    }

    fn create_pipeline(
        device: &wgpu::Device,
        shader: &ShaderModule,
        color_format: wgpu::TextureFormat,
    ) -> RenderPipeline {
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("diffuse irradiance baking pipeline layout"),
            bind_group_layouts: &[
                &device.create_bind_group_layout(
                    &bind_group_layout_descriptors::BUFFER_VISIBLE_EVERYWHERE,
                ),
                &device.create_bind_group_layout(
                    &bind_group_layout_descriptors::TEXTURE_CUBE_FRAGMENT_COMPUTE_WITH_SAMPLER,
                ),
            ],
            push_constant_ranges: &[],
        });

        // Create the render pipeline
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("diffuse irradiance baking render pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                compilation_options: PipelineCompilationOptions::default(),
                module: shader,
                entry_point: Some("vs_main"),
                buffers: &[vertex::VertexRawWithTangents::buffer_layout()],
            },
            fragment: Some(FragmentState {
                compilation_options: PipelineCompilationOptions::default(),
                module: shader,
                entry_point: Some("fs_main"),
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
            cache: None,
        })
    }

    pub fn try_recompile_shader(
        &mut self,
        device: &Device,
        color_format: wgpu::TextureFormat,
    ) -> anyhow::Result<ShaderCompilationSuccess> {
        let result = self.shader_compiler.compile_shader_if_needed(device)?;

        match result {
            ShaderCompilationResult::AlreadyUpToDate => {
                Ok(ShaderCompilationSuccess::AlreadyUpToDate)
            }
            ShaderCompilationResult::Success(shader_module) => {
                let pipeline = Self::create_pipeline(device, &shader_module, color_format);
                self.render_pipeline = pipeline;
                Ok(ShaderCompilationSuccess::Recompiled)
            }
        }
    }

    pub fn render(
        &self,
        encoder: &mut CommandEncoder,
        color_target: &wgpu::TextureView,
        primitive: &Primitive,
        projection_bind_group: &wgpu::BindGroup,
        hdr_environment_cube_map: &wgpu::BindGroup,
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Diffuse irradiance baking pass"),
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
        render_pass.set_bind_group(1, hdr_environment_cube_map, &[]);

        render_pass.set_vertex_buffer(0, primitive.vertex_buffer.slice(..));
        render_pass.set_index_buffer(
            primitive.index_data.buffer.slice(..),
            wgpu::IndexFormat::Uint32,
        );
        render_pass.draw_indexed(0..primitive.index_data.count, 0, 0..1 as u32);
    }
}
