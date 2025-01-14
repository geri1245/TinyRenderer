use wgpu::{
    BindGroup, Device, PipelineCompilationOptions, RenderPass, RenderPipeline, ShaderModule,
};

use crate::{
    bind_group_layout_descriptors,
    buffer_content::BufferContent,
    instance,
    model::Renderable,
    texture::{self, SampledTexture},
    vertex,
};

use super::shader_compiler::{ShaderCompilationResult, ShaderCompilationSuccess, ShaderCompiler};

const SHADER_SOURCE_TEXTURED: &'static str = "src/shaders/gbuffer_geometry.wgsl";
const SHADER_SOURCE_FLAT_PARAMETER: &'static str =
    "src/shaders/gbuffer_geometry_flat_parameter.wgsl";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum PbrParameterVariation {
    Texture, // All the parameters are given as textures
    Flat,    // The parameters are given as plain old numbers
}

pub struct GBufferTextures {
    pub position: SampledTexture,
    pub normal: SampledTexture,
    pub albedo_and_specular: SampledTexture,
    pub depth_texture: SampledTexture,
    pub metal_rough_ao: SampledTexture,
}

pub struct GBufferGeometryRP {
    render_pipeline: wgpu::RenderPipeline,
    shader_compiler: ShaderCompiler,
}

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
        variation: PbrParameterVariation,
    ) -> RenderPipeline {
        let pbr_texture_bind_group =
            device.create_bind_group_layout(&bind_group_layout_descriptors::PBR_TEXTURE);
        let buffer_bind_group = device
            .create_bind_group_layout(&bind_group_layout_descriptors::BUFFER_VISIBLE_EVERYWHERE);

        let pipeline_layout = match variation {
            PbrParameterVariation::Texture => {
                device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Geometry pass pipeline layout, textured"),
                    bind_group_layouts: &[
                        &pbr_texture_bind_group,
                        &buffer_bind_group,
                        &buffer_bind_group,
                    ],
                    push_constant_ranges: &[],
                })
            }
            PbrParameterVariation::Flat => {
                device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Geometry pass pipeline layout, flat parameters"),
                    bind_group_layouts: &[
                        &buffer_bind_group,
                        &buffer_bind_group,
                        &buffer_bind_group,
                    ],
                    push_constant_ranges: &[],
                })
            }
        };

        let gbuffer_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gbuffer pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                compilation_options: PipelineCompilationOptions::default(),
                module: shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    vertex::VertexRawWithTangents::buffer_layout(),
                    instance::SceneComponentRaw::buffer_layout(),
                ],
            },
            fragment: Some(wgpu::FragmentState {
                compilation_options: PipelineCompilationOptions::default(),
                module: &shader,
                entry_point: Some("fs_main"),
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
                depth_compare: wgpu::CompareFunction::Greater,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        gbuffer_pipeline
    }

    pub fn new(
        device: &wgpu::Device,
        gbuffer_textures: &GBufferTextures,
        variation: PbrParameterVariation,
    ) -> anyhow::Result<Self> {
        let source = match variation {
            PbrParameterVariation::Texture => SHADER_SOURCE_TEXTURED,
            PbrParameterVariation::Flat => SHADER_SOURCE_FLAT_PARAMETER,
        };
        let mut shader_compiler = ShaderCompiler::new(source.to_string());
        let shader_compilation_result = shader_compiler.compile_shader_if_needed(device)?;

        match shader_compilation_result {
            ShaderCompilationResult::AlreadyUpToDate => {
                panic!("This shader hasn't been compiled yet, can't be up to date!")
            }
            ShaderCompilationResult::Success(shader) => Ok(Self {
                render_pipeline: Self::create_pipeline(
                    device,
                    &shader,
                    gbuffer_textures,
                    variation,
                ),
                shader_compiler,
            }),
        }
    }

    pub fn try_recompile_shader(
        &mut self,
        device: &Device,
        textures: &GBufferTextures,
        variation: PbrParameterVariation,
    ) -> anyhow::Result<ShaderCompilationSuccess> {
        let result = self.shader_compiler.compile_shader_if_needed(device)?;

        match result {
            ShaderCompilationResult::AlreadyUpToDate => {
                Ok(ShaderCompilationSuccess::AlreadyUpToDate)
            }
            ShaderCompilationResult::Success(shader_module) => {
                let pipeline = Self::create_pipeline(device, &shader_module, textures, variation);
                self.render_pipeline = pipeline;
                Ok(ShaderCompilationSuccess::Recompiled)
            }
        }
    }

    pub fn render<'a, T: Iterator<Item = &'a Renderable>>(
        &'a self,
        render_pass: &mut RenderPass<'a>,
        renderables: T,
        camera_bind_group: &'a BindGroup,
        global_gpu_params_bind_group: &'a BindGroup,
    ) {
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(1, camera_bind_group, &[]);
        render_pass.set_bind_group(2, global_gpu_params_bind_group, &[]);

        for renderable in renderables {
            renderable.render(render_pass, Some(0));
        }
    }
}
