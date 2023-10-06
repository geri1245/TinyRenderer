use wgpu::{BindGroup, Buffer, RenderPass};

use crate::{
    bind_group_layout_descriptors,
    buffer_content::BufferContent,
    instance,
    model::{Mesh, Model},
    texture, vertex,
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
                source: wgpu::ShaderSource::Wgsl(
                    include_str!("../shaders/forward_pass_with_color.wgsl").into(),
                ),
            };
            let shader = device.create_shader_module(shader);

            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Forward render pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[
                        vertex::VertexRaw::buffer_layout(),
                        instance::InstanceRaw::buffer_layout(),
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
                    format: texture::Texture::DEPTH_FORMAT,
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
        model: &'a Model,
        camera_bind_group: &'a BindGroup,
        light_bind_group: &'a BindGroup,
        instances: usize,
        instance_buffer: &'a Buffer,
    ) {
        self.prepare_render(
            render_pass,
            camera_bind_group,
            light_bind_group,
            instance_buffer,
        );
        for mesh in &model.meshes {
            self.render_mesh_internal(render_pass, mesh, instances);
        }
    }

    pub fn _render_mesh<'a>(
        &'a self,
        render_pass: &mut RenderPass<'a>,
        mesh: &'a Mesh,
        camera_bind_group: &'a BindGroup,
        light_bind_group: &'a BindGroup,
        instances: usize,
        instance_buffer: &'a Buffer,
    ) {
        self.prepare_render(
            render_pass,
            camera_bind_group,
            light_bind_group,
            instance_buffer,
        );
        self.render_mesh_internal(render_pass, mesh, instances);
    }

    fn prepare_render<'a>(
        &'a self,
        render_pass: &mut RenderPass<'a>,
        camera_bind_group: &'a BindGroup,
        light_bind_group: &'a BindGroup,
        instance_buffer: &'a Buffer,
    ) {
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &light_bind_group, &[]);
        render_pass.set_bind_group(1, &camera_bind_group, &[]);
        render_pass.set_vertex_buffer(1, instance_buffer.slice(..));
    }

    fn render_mesh_internal<'a>(
        &self,
        render_pass: &mut RenderPass<'a>,
        mesh: &'a Mesh,
        instances: usize,
    ) {
        render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..mesh.index_count, 0, 0..instances as u32);
    }
}
