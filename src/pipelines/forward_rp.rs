use wgpu::{
    BindGroup, Device, PipelineCompilationOptions, RenderPass, RenderPipeline, ShaderModule,
};

use crate::{
    bind_group_layout_descriptors, buffer_content::BufferContent, instance, model::Renderable,
    texture, vertex,
};

use super::{
    shader_compiler::{ShaderCompilationResult, ShaderCompiler},
    ShaderCompilationSuccess,
};

const SHADER_SOURCE: &'static str = "src/shaders/forward.wgsl";

pub struct ForwardRP {
    render_pipeline: wgpu::RenderPipeline,
    shader_compiler: ShaderCompiler,
}

impl ForwardRP {
    pub async fn new(
        device: &wgpu::Device,
        texture_format: wgpu::TextureFormat,
    ) -> anyhow::Result<Self> {
        let mut shader_compiler = ShaderCompiler::new(SHADER_SOURCE);
        let shader_compilation_result = shader_compiler.compile_shader_if_needed(device).await?;

        match shader_compilation_result {
            ShaderCompilationResult::AlreadyUpToDate => {
                panic!("This shader hasn't been compiled yet, can't be up to date!")
            }
            ShaderCompilationResult::Success(shader) => Ok(Self {
                render_pipeline: Self::create_pipeline(device, &shader, texture_format),
                shader_compiler,
            }),
        }
    }

    pub async fn try_recompile_shader(
        &mut self,
        device: &Device,
        texture_format: wgpu::TextureFormat,
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
                let pipeline = Self::create_pipeline(device, &shader_module, texture_format);
                self.render_pipeline = pipeline;
                Ok(ShaderCompilationSuccess::Recompiled)
            }
        }
    }

    fn create_pipeline(
        device: &wgpu::Device,
        shader: &ShaderModule,
        texture_format: wgpu::TextureFormat,
    ) -> RenderPipeline {
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Light Pipeline Layout"),
            bind_group_layouts: &[
                &device.create_bind_group_layout(&bind_group_layout_descriptors::LIGHT),
                &device.create_bind_group_layout(
                    &bind_group_layout_descriptors::BUFFER_VISIBLE_EVERYWHERE,
                ),
            ],
            push_constant_ranges: &[],
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Forward render pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                compilation_options: PipelineCompilationOptions::default(),
                module: shader,
                entry_point: "vs_main",
                buffers: &[
                    vertex::VertexRawWithTangents::buffer_layout(),
                    instance::SceneComponentRaw::buffer_layout(),
                ],
            },
            fragment: Some(wgpu::FragmentState {
                compilation_options: PipelineCompilationOptions::default(),
                module: shader,
                entry_point: "fs_main",
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
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    }

    pub fn render_model<'a, T: Iterator<Item = &'a Renderable>>(
        &'a self,
        render_pass: &mut RenderPass<'a>,
        renderables: T,
        camera_bind_group: &'a BindGroup,
        light_bind_group: &'a BindGroup,
    ) {
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &light_bind_group, &[]);
        render_pass.set_bind_group(1, &camera_bind_group, &[]);

        for renderable in renderables {
            renderable.render(render_pass, false);
        }
    }
}
