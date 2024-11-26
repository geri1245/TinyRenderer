use wgpu::{
    BindGroup, ColorTargetState, DepthStencilState, Device, Face, FragmentState,
    PipelineCompilationOptions, RenderPass, RenderPipeline, ShaderModule, TextureFormat,
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

// TODO: this double render pipeline solution won't scale well when other parameters are introduced
// Instead of defining fix pipelines, they should be generated on the fly. If we encounter a model that we
// can't render with the existing pipelines, then we should generate a new one for it and then store it
// Some time-based LRU cache can also be introduced to remove pipelines that aren't used for a long time
// It's also worth considering if this RenderPipeline struct should hold multiple wgpu::RenderPipeline objects
// Or this should hold only a single one and the containing class should hold multiple ObjectPickerRPs
pub struct ObjectPickerRP {
    pub render_pipeline: wgpu::RenderPipeline,
    pub render_pipeline_no_depth_test: wgpu::RenderPipeline,
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
                render_pipeline: Self::create_pipeline(
                    device,
                    &shader,
                    color_format,
                    depth_format,
                    true,
                ),
                render_pipeline_no_depth_test: Self::create_pipeline(
                    device,
                    &shader,
                    color_format,
                    depth_format,
                    false,
                ),
                shader_compiler,
            }),
        }
    }

    fn create_pipeline(
        device: &wgpu::Device,
        shader: &ShaderModule,
        color_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
        use_depth_test: bool,
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
                depth_compare: if use_depth_test {
                    wgpu::CompareFunction::Equal
                } else {
                    wgpu::CompareFunction::Always
                },
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
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
                    Self::create_pipeline(device, &shader_module, color_format, depth_format, true);
                self.render_pipeline = pipeline;
                let pipeline_no_depth_test = Self::create_pipeline(
                    device,
                    &shader_module,
                    color_format,
                    depth_format,
                    false,
                );
                self.render_pipeline_no_depth_test = pipeline_no_depth_test;
                Ok(ShaderCompilationSuccess::Recompiled)
            }
        }
    }

    pub fn render<'a, T>(
        &'a self,
        render_pass: &mut RenderPass<'a>,
        renderables: T,
        camera_bind_group: &'a BindGroup,
    ) where
        T: Iterator<Item = &'a Renderable> + Clone,
    {
        render_pass.set_bind_group(0, camera_bind_group, &[]);

        render_pass.set_pipeline(&self.render_pipeline);

        for renderable in renderables
            .clone()
            .clone()
            .filter(|renderable| renderable.description.rendering_options.use_depth_test)
        {
            renderable.render(render_pass, false);
        }

        render_pass.set_pipeline(&self.render_pipeline_no_depth_test);

        for renderable in renderables
            .clone()
            .clone()
            .filter(|renderable| !renderable.description.rendering_options.use_depth_test)
        {
            renderable.render(render_pass, false);
        }
    }
}
