use wgpu::{
    BindGroup, CommandEncoder, Device, PipelineCompilationOptions,
    RenderPassDepthStencilAttachment, RenderPipeline, ShaderModule, TextureFormat,
};

use crate::{
    bind_group_layout_descriptors, buffer_content::BufferContent, instance, model::Renderable,
    vertex,
};

use super::shader_compiler::{ShaderCompilationResult, ShaderCompilationSuccess, ShaderCompiler};

const SHADER_SOURCE: &'static str = "src/shaders/shadow.wgsl";
// TODO: share this with the shadow code, don't define this again
const DEPTH_FORMAT: TextureFormat = TextureFormat::Depth32Float;

pub struct ShadowRP {
    pipeline: wgpu::RenderPipeline,
    shader_compiler: ShaderCompiler,
}

impl ShadowRP {
    pub async fn new(device: &wgpu::Device) -> anyhow::Result<Self> {
        let mut shader_compiler = ShaderCompiler::new(SHADER_SOURCE.to_string());
        let shader_compilation_result = shader_compiler.compile_shader_if_needed(device).await?;

        match shader_compilation_result {
            ShaderCompilationResult::AlreadyUpToDate => {
                panic!("This shader hasn't been compiled yet, can't be up to date!")
            }
            ShaderCompilationResult::Success(shader) => Ok(Self {
                pipeline: Self::create_pipeline(device, &shader),
                shader_compiler,
            }),
        }
    }

    pub async fn try_recompile_shader(
        &mut self,
        device: &Device,
    ) -> anyhow::Result<ShaderCompilationSuccess> {
        let result = self
            .shader_compiler
            .compile_shader_if_needed(device)
            .await?;

        match result {
            ShaderCompilationResult::AlreadyUpToDate => {
                Ok(ShaderCompilationSuccess::AlreadyUpToDate)
            }
            ShaderCompilationResult::Success(shader_module) => {
                let pipeline = Self::create_pipeline(device, &shader_module);
                self.pipeline = pipeline;
                Ok(ShaderCompilationSuccess::Recompiled)
            }
        }
    }

    fn create_pipeline(device: &wgpu::Device, shader: &ShaderModule) -> RenderPipeline {
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shadow pipeline layout"),
            bind_group_layouts: &[&device.create_bind_group_layout(
                &(bind_group_layout_descriptors::BUFFER_WITH_DYNAMIC_OFFSET),
            )],
            push_constant_ranges: &[],
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow render pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                compilation_options: PipelineCompilationOptions::default(),
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
                depth_compare: wgpu::CompareFunction::Greater,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: 0,
                    slope_scale: 0.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    }

    pub fn render<'a, T: Iterator<Item = &'a Renderable>>(
        &self,
        encoder: &mut CommandEncoder,
        renderables: T,
        light_bind_group: &BindGroup,
        depth_target: &wgpu::TextureView,
        light_bind_group_offset: u32,
    ) {
        let mut shadow_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: depth_target,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        shadow_pass.set_pipeline(&self.pipeline);
        shadow_pass.set_bind_group(0, &light_bind_group, &[light_bind_group_offset]);

        for renderable in
            renderables.filter(|renderable| renderable.description.rendering_options.cast_shadows)
        {
            renderable.render(&mut shadow_pass, None);
        }
    }
}
