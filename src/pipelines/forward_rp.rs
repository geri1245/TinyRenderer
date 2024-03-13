use wgpu::{BindGroup, RenderPass};

use crate::{
    bind_group_layout_descriptors, buffer_content::BufferContent, instance,
    model::InstancedRenderableMesh, texture, vertex,
};

pub struct ForwardRP {
    render_pipeline: wgpu::RenderPipeline,
}

impl ForwardRP {
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        let render_pipeline = {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Light Pipeline Layout"),
                bind_group_layouts: &[
                    &device.create_bind_group_layout(&bind_group_layout_descriptors::LIGHT),
                    &device.create_bind_group_layout(&bind_group_layout_descriptors::CAMERA),
                ],
                push_constant_ranges: &[],
            });
            let shader = wgpu::ShaderModuleDescriptor {
                label: Some("Light Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/forward.wgsl").into()),
            };
            let shader = device.create_shader_module(shader);

            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Forward render pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[
                        vertex::VertexRawWithTangents::buffer_layout(),
                        instance::SceneComponentRaw::buffer_layout(),
                    ],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: color_format,
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
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            })
        };

        ForwardRP { render_pipeline }
    }

    pub fn render_model<'a>(
        &'a self,
        render_pass: &mut RenderPass<'a>,
        mesh: &'a InstancedRenderableMesh,
        camera_bind_group: &'a BindGroup,
        light_bind_group: &'a BindGroup,
    ) {
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &light_bind_group, &[]);
        render_pass.set_bind_group(1, &camera_bind_group, &[]);
        render_pass.set_vertex_buffer(1, mesh.instance_buffer.slice(..));

        render_pass.set_vertex_buffer(0, mesh.mesh.vertex_buffer.slice(..));
        render_pass.set_index_buffer(mesh.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..mesh.mesh.index_count, 0, 0..mesh.instances.len() as u32);
    }
}
