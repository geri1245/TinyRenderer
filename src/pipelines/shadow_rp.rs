use wgpu::{BindGroup, Buffer, CommandEncoder, RenderPassDepthStencilAttachment};

use crate::{
    bind_group_layout_descriptors, buffer_content::BufferContent, instance, model::Model,
    texture::Texture, vertex,
};

const SHADOW_SIZE: wgpu::Extent3d = wgpu::Extent3d {
    width: 4096,
    height: 4096,
    depth_or_array_layers: crate::renderer::MAX_LIGHTS as u32,
};

pub struct ShadowRP {
    shadow_target_view: wgpu::TextureView,
    shadow_pipeline: wgpu::RenderPipeline,
    pub bind_group: wgpu::BindGroup,
}

impl ShadowRP {
    pub fn new(device: &wgpu::Device) -> ShadowRP {
        let shadow_texture = Texture::create_depth_texture(
            device,
            SHADOW_SIZE.width,
            SHADOW_SIZE.height,
            "Shadow texture",
        );
        let shadow_view = shadow_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let shadow_target_view = shadow_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor {
                label: Some("shadow depth texture"),
                format: None,
                dimension: Some(wgpu::TextureViewDimension::D2),
                aspect: wgpu::TextureAspect::All,
                base_mip_level: 0,
                mip_level_count: None,
                base_array_layer: 0,
                array_layer_count: Some(1),
            });

        let shadow_pipeline = {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("shadow pipeline layout"),
                bind_group_layouts: &[
                    &device.create_bind_group_layout(&(bind_group_layout_descriptors::LIGHT))
                ],
                push_constant_ranges: &[],
            });

            let shadow_shader_desc = wgpu::ShaderModuleDescriptor {
                label: Some("Shadow bake shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/shadow.wgsl").into()),
            };

            let shadow_shader = device.create_shader_module(shadow_shader_desc);

            // Create the render pipeline
            let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("shadow render pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shadow_shader,
                    entry_point: "vs_bake",
                    buffers: &[
                        vertex::VertexRaw::buffer_layout(),
                        instance::InstanceRaw::buffer_layout(),
                    ],
                },
                fragment: None,
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    unclipped_depth: device
                        .features()
                        .contains(wgpu::Features::DEPTH_CLIP_CONTROL),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: shadow_texture.format,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState {
                        constant: 2, // corresponds to bilinear filtering
                        slope_scale: 2.0,
                        clamp: 0.0,
                    },
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

            shadow_pipeline
        };

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &device.create_bind_group_layout(&bind_group_layout_descriptors::DEPTH_TEXTURE),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&shadow_texture.sampler),
                },
            ],
            label: None,
        });

        ShadowRP {
            bind_group,
            shadow_pipeline,
            shadow_target_view,
        }
    }

    pub fn render(
        &self,
        encoder: &mut CommandEncoder,
        model: &Model,
        light_bind_group: &BindGroup,
        instances: usize,
        instance_buffer: &Buffer,
    ) {
        let mut shadow_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Shadow pass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: &self.shadow_target_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        shadow_pass.set_pipeline(&self.shadow_pipeline);

        shadow_pass.set_bind_group(0, &light_bind_group, &[]);

        shadow_pass.set_vertex_buffer(1, instance_buffer.slice(..));

        for mesh in &model.meshes {
            shadow_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            shadow_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            shadow_pass.draw_indexed(0..mesh.index_count, 0, 0..instances as u32);
        }
    }
}
