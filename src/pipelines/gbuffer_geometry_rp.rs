use wgpu::{BindGroup, Device, RenderPass, RenderPipeline, ShaderModule};

use crate::{
    bind_group_layout_descriptors,
    buffer_content::BufferContent,
    instance,
    model::InstancedTexturedRenderableMesh,
    texture::{self, SampledTexture},
    vertex,
};

use super::shader_compiler::{ShaderCompilationResult, ShaderCompiler};

const SHADER_SOURCE: &'static str = "src/shaders/gbuffer_geometry.wgsl";

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
    ) -> RenderPipeline {
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Geometry pass pipeline layout"),
            bind_group_layouts: &[
                &device.create_bind_group_layout(&bind_group_layout_descriptors::PBR_TEXTURE),
                &device.create_bind_group_layout(
                    &bind_group_layout_descriptors::BUFFER_VISIBLE_EVERYWHERE,
                ),
            ],
            push_constant_ranges: &[],
        });

        let gbuffer_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gbuffer pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: "vs_main",
                buffers: &[
                    vertex::VertexRawWithTangents::buffer_layout(),
                    instance::SceneComponentRaw::buffer_layout(),
                ],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
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
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        gbuffer_pipeline
    }

    pub async fn new(
        device: &wgpu::Device,
        gbuffer_textures: &GBufferTextures,
    ) -> anyhow::Result<Self> {
        let mut shader_compiler = ShaderCompiler::new(SHADER_SOURCE);
        let shader_compilation_result = shader_compiler.compile_shader_if_needed(device).await?;

        match shader_compilation_result {
            ShaderCompilationResult::AlreadyUpToDate => {
                panic!("This shader hasn't been compiled yet, can't be up to date!")
            }
            ShaderCompilationResult::Success(shader) => Ok(Self {
                render_pipeline: Self::create_pipeline(device, &shader, gbuffer_textures),
                shader_compiler,
            }),
        }
    }

    pub async fn try_recompile_shader(
        &mut self,
        device: &Device,
        textures: &GBufferTextures,
    ) -> anyhow::Result<()> {
        let result = self
            .shader_compiler
            .compile_shader_if_needed(device)
            .await?;

        match result {
            ShaderCompilationResult::AlreadyUpToDate => Ok(()),
            ShaderCompilationResult::Success(shader_module) => {
                let pipeline = Self::create_pipeline(device, &shader_module, textures);
                self.render_pipeline = pipeline;
                Ok(())
            }
        }
    }

    pub fn render_mesh<'a>(
        &'a self,
        render_pass: &mut RenderPass<'a>,
        mesh: &'a InstancedTexturedRenderableMesh,
        camera_bind_group: &'a BindGroup,
    ) {
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(1, &camera_bind_group, &[]);
        render_pass.set_vertex_buffer(1, mesh.mesh.instance_buffer.slice(..));

        render_pass.set_bind_group(0, &mesh.material.bind_group, &[]);
        render_pass.set_vertex_buffer(0, mesh.mesh.mesh.vertex_buffer.slice(..));
        render_pass.set_index_buffer(
            mesh.mesh.mesh.index_buffer.slice(..),
            wgpu::IndexFormat::Uint32,
        );
        render_pass.draw_indexed(
            0..mesh.mesh.mesh.index_count,
            0,
            0..mesh.mesh.instances.len() as u32,
        );
    }
}
