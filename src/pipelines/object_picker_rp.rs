use wgpu::{
    ColorTargetState, DepthStencilState, Device, Face, FragmentState, PipelineCompilationOptions,
    RenderPass, RenderPipeline, ShaderModule, TextureFormat,
};

use crate::{
    bind_group_layout_descriptors, buffer_content::BufferContent, instance, model::Renderable,
    vertex,
};

use super::{
    shader_compiler::{ShaderCompilationResult, ShaderCompiler},
    ShaderCompilationSuccess,
};

const SHADER_SOURCE: &'static str = "src/shaders/pick.wgsl";

pub struct ObjectPickerRP {
    pub render_pipeline: wgpu::RenderPipeline,
    shader_compiler: ShaderCompiler,
}

impl ObjectPickerRP {
    pub async fn new(
        device: &wgpu::Device,
        color_format: TextureFormat,
        depth_format: wgpu::TextureFormat,
    ) -> anyhow::Result<Self> {
        let mut shader_compiler = ShaderCompiler::new(SHADER_SOURCE);
        let shader_compilation_result = shader_compiler.compile_shader_if_needed(device).await?;

        match shader_compilation_result {
            ShaderCompilationResult::AlreadyUpToDate => {
                panic!("This shader hasn't been compiled yet, can't be up to date!")
            }
            ShaderCompilationResult::Success(shader) => Ok(Self {
                render_pipeline: Self::create_pipeline(device, &shader, color_format, depth_format),
                shader_compiler,
            }),
        }
    }

    fn create_pipeline(
        device: &wgpu::Device,
        shader: &ShaderModule,
        color_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
    ) -> RenderPipeline {
        let buffer_bind_group = device
            .create_bind_group_layout(&bind_group_layout_descriptors::BUFFER_VISIBLE_EVERYWHERE);

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Object pick pipeline layout"),
            bind_group_layouts: &[&buffer_bind_group],
            push_constant_ranges: &[],
        });

        // Create the render pipeline
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Object picking render pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                compilation_options: PipelineCompilationOptions::default(),
                module: shader,
                entry_point: "vs_pick_main",
                buffers: &[
                    vertex::VertexRawWithTangents::buffer_layout(),
                    instance::SceneComponentRaw::buffer_layout(),
                ],
            },
            fragment: Some(FragmentState {
                compilation_options: PipelineCompilationOptions::default(),
                module: shader,
                entry_point: "fs_pick_main",
                targets: &[Some(ColorTargetState {
                    format: color_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(DepthStencilState {
                format: depth_format,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Equal,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        })
    }

    pub async fn try_recompile_shader(
        &mut self,
        device: &Device,
        color_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
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
                let pipeline =
                    Self::create_pipeline(device, &shader_module, color_format, depth_format);
                self.render_pipeline = pipeline;
                Ok(ShaderCompilationSuccess::Recompiled)
            }
        }
    }

    pub fn render<'a>(&'a self, render_pass: &mut RenderPass<'a>, renderable: &'a Renderable) {
        renderable.render(render_pass, false);
    }
}
