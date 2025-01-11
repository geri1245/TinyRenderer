use wgpu::{
    BindGroup, BindGroupLayout, ColorTargetState, DepthStencilState, Device, Face, FragmentState,
    FrontFace, PipelineCompilationOptions, PipelineLayoutDescriptor, PolygonMode, PrimitiveState,
    PrimitiveTopology, RenderPass, ShaderModule, TextureFormat, VertexBufferLayout,
};

use crate::{
    buffer_content::BufferContent,
    instance::SceneComponentRaw,
    model::Renderable,
    pipelines::{ShaderCompilationResult, ShaderCompilationSuccess, ShaderCompiler},
    texture,
    vertex::VertexRawWithTangents,
};

pub struct PipelineVertexState {
    pub entry_point: &'static str,
    pub vertex_layouts: Vec<VertexBufferContent>,
}

impl Default for PipelineVertexState {
    fn default() -> Self {
        Self {
            entry_point: "vs_main",
            vertex_layouts: vec![VertexBufferContent::VertexWithTangent],
        }
    }
}

pub struct PipelineFragmentState {
    pub entry_point: &'static str,
    pub color_targets: Vec<ColorTargetState>,
}

impl Default for PipelineFragmentState {
    fn default() -> Self {
        Self {
            entry_point: "fs_main",
            color_targets: vec![ColorTargetState {
                format: TextureFormat::Bgra8Unorm, // TODO: this should be set from the outside
                blend: Some(wgpu::BlendState {
                    alpha: wgpu::BlendComponent::REPLACE,
                    color: wgpu::BlendComponent::REPLACE,
                }),
                write_mask: wgpu::ColorWrites::ALL,
            }],
        }
    }
}

pub enum VertexBufferContent {
    VertexWithTangent,
    SceneComponent,
}

impl VertexBufferContent {
    fn to_vertex_buffer_layout(&self) -> VertexBufferLayout {
        match self {
            VertexBufferContent::VertexWithTangent => VertexRawWithTangents::buffer_layout(),
            VertexBufferContent::SceneComponent => SceneComponentRaw::buffer_layout(),
        }
    }
}

pub struct RenderPipelineDescriptor {
    /// Debug label of the pipeline. This will show up in graphics debuggers for easy identification.
    pub name: Option<String>,
    pub shader_source_path: String,
    /// The compiled vertex stage, its entry point, and the input buffers layout.
    pub vertex: PipelineVertexState,
    /// The properties of the pipeline at the primitive assembly and rasterization level.
    pub primitive: PrimitiveState,
    /// The effect of draw calls on the depth and stencil aspects of the output target, if any.
    pub depth_stencil: Option<DepthStencilState>,
    /// The compiled fragment stage, its entry point, and the color targets.
    pub fragment: PipelineFragmentState,
    /// The list of bind group layouts of this pipeline
    pub bind_group_layouts: Vec<BindGroupLayout>,
    /// Which bind group slot the material should be bound. If no material is used, then this should be None
    pub material_bind_group_index: Option<u32>, // TODO: Remove this from here, doesn't belong here
                                                // Some more general method would be needed for communicating to the pipeline how the renderables should be rendered
                                                // Maybe pass in a callback?
}

impl Default for RenderPipelineDescriptor {
    fn default() -> Self {
        Self {
            name: None,
            vertex: PipelineVertexState::default(),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Back),
                unclipped_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: texture::SampledTexture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Greater,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            fragment: PipelineFragmentState::default(),
            shader_source_path: "".to_string(),
            bind_group_layouts: vec![],
            material_bind_group_index: None,
        }
    }
}

pub struct RenderPipeline {
    render_pipeline: wgpu::RenderPipeline,
    shader_compiler: ShaderCompiler,
    descriptor: RenderPipelineDescriptor,
}

impl RenderPipeline {
    pub fn new(
        device: &wgpu::Device,
        descriptor: RenderPipelineDescriptor,
    ) -> anyhow::Result<Self> {
        let mut shader_compiler = ShaderCompiler::new(descriptor.shader_source_path.clone());
        let shader_compilation_result = shader_compiler.compile_shader_if_needed(device)?;

        match shader_compilation_result {
            ShaderCompilationResult::AlreadyUpToDate => {
                panic!("This shader hasn't been compiled yet, can't be up to date!")
            }
            ShaderCompilationResult::Success(shader) => {
                let render_pipeline = Self::create_pipeline(device, &shader, &descriptor);

                Ok(Self {
                    render_pipeline,
                    shader_compiler,
                    descriptor,
                })
            }
        }
    }

    fn create_pipeline(
        device: &wgpu::Device,
        shader: &ShaderModule,
        desc: &RenderPipelineDescriptor,
    ) -> wgpu::RenderPipeline {
        let label = desc.name.clone().unwrap_or(desc.shader_source_path.clone());
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some(&format!("{label} pipeline layout")),
            bind_group_layouts: &desc
                .bind_group_layouts
                .iter()
                .map(|lay| lay)
                .collect::<Vec<_>>(),
            push_constant_ranges: &[],
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&format!("{label} render pipeline")),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some(&desc.vertex.entry_point),
                buffers: &desc
                    .vertex
                    .vertex_layouts
                    .iter()
                    .map(|layout| layout.to_vertex_buffer_layout())
                    .collect::<Vec<_>>(),
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(FragmentState {
                module: shader,
                entry_point: Some(&desc.fragment.entry_point),
                targets: &desc
                    .fragment
                    .color_targets
                    .iter()
                    .map(|target| Some(target.clone()))
                    .collect::<Vec<_>>(),
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: desc.primitive,
            depth_stencil: desc.depth_stencil.clone(),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    }

    pub fn try_recompile_shader(
        &mut self,
        device: &Device,
    ) -> anyhow::Result<ShaderCompilationSuccess> {
        let result = self
            .shader_compiler
            .compile_shader_if_needed(device)
            ?;

        match result {
            ShaderCompilationResult::AlreadyUpToDate => {
                Ok(ShaderCompilationSuccess::AlreadyUpToDate)
            }
            ShaderCompilationResult::Success(shader_module) => {
                let pipeline = Self::create_pipeline(device, &shader_module, &self.descriptor);
                self.render_pipeline = pipeline;
                Ok(ShaderCompilationSuccess::Recompiled)
            }
        }
    }

    fn set_render_parameters<'a>(
        &self,
        render_pass: &mut RenderPass<'a>,
        bind_groups: &[&'a BindGroup],
    ) {
        render_pass.set_pipeline(&self.render_pipeline);

        for (index, bind_group) in bind_groups.iter().enumerate() {
            render_pass.set_bind_group(index as u32, *bind_group, &[]);
        }
    }

    pub fn render<'a, T: Iterator<Item = &'a Renderable> + Clone>(
        &'a self,
        render_pass: &mut RenderPass<'a>,
        bind_groups: &[&'a BindGroup],
        renderables: T,
    ) {
        self.set_render_parameters(render_pass, bind_groups);
        for renderable in renderables {
            renderable.render(render_pass, self.descriptor.material_bind_group_index);
        }
    }
}
