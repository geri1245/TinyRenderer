use wgpu::{BindGroup, Device, RenderPass};

use crate::{
    bind_group_layout_descriptors,
    model::Renderable,
    pipelines::ShaderCompilationSuccess,
    render_pipeline::{
        PipelineFragmentState, PipelineVertexState, RenderPipeline, RenderPipelineDescriptor,
        VertexBufferContent,
    },
    texture::SampledTexture,
};

const SHADER_SOURCE: &'static str = "src/shaders/forward.wgsl";

pub struct ForwardRenderer {
    pipeline: RenderPipeline,
}

impl ForwardRenderer {
    pub fn new(device: &wgpu::Device, texture_format: wgpu::TextureFormat) -> Self {
        let bind_group_layouts = vec![
            device.create_bind_group_layout(&bind_group_layout_descriptors::LIGHT),
            device.create_bind_group_layout(
                &bind_group_layout_descriptors::BUFFER_VISIBLE_EVERYWHERE,
            ),
            device.create_bind_group_layout(
                &bind_group_layout_descriptors::BUFFER_VISIBLE_EVERYWHERE,
            ),
        ];
        let pipeline = RenderPipeline::new(
            device,
            RenderPipelineDescriptor {
                name: Some("Forward".to_string()),
                shader_source_path: SHADER_SOURCE.to_string(),
                vertex: PipelineVertexState {
                    vertex_layouts: vec![
                        VertexBufferContent::VertexWithTangent,
                        VertexBufferContent::TransformComponent,
                    ],
                    ..Default::default()
                },
                primitive: Default::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: SampledTexture::DEPTH_FORMAT,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Always,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                fragment: PipelineFragmentState {
                    color_targets: vec![wgpu::ColorTargetState {
                        format: texture_format,
                        blend: Some(wgpu::BlendState {
                            alpha: wgpu::BlendComponent::REPLACE,
                            color: wgpu::BlendComponent::REPLACE,
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    }],
                    ..Default::default()
                },
                bind_group_layouts,
                material_bind_group_index: Some(2),
            },
        )
        .unwrap();

        Self { pipeline }
    }

    pub fn try_recompile_shader(
        &mut self,
        device: &Device,
    ) -> anyhow::Result<ShaderCompilationSuccess> {
        self.pipeline.try_recompile_shader(device)
    }

    pub fn render<'a, T: Iterator<Item = &'a Renderable> + Clone>(
        &'a self,
        render_pass: &mut RenderPass<'a>,
        renderables: T,
        camera_bind_group: &'a BindGroup,
        light_bind_group: &'a BindGroup,
    ) {
        self.pipeline.render(
            render_pass,
            &[light_bind_group, camera_bind_group],
            renderables,
            0,
        );
    }
}
