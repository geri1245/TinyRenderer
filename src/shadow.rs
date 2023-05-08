use std::{collections::HashMap, num::NonZeroU32};

use wgpu::{BindGroup, Buffer, RenderPassDepthStencilAttachment};

use crate::{
    buffer_content::BufferContent, instance, model::Model, renderer::BindGroupLayoutType, vertex,
};

const SHADOW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const SHADOW_SIZE: wgpu::Extent3d = wgpu::Extent3d {
    width: 1024,
    height: 1024,
    depth_or_array_layers: crate::renderer::MAX_LIGHTS as u32,
};

pub struct Shadow {
    shadow_target_views: Vec<wgpu::TextureView>,
    shadow_pipeline: wgpu::RenderPipeline,
    pub bind_group: wgpu::BindGroup,
}

impl Shadow {
    pub fn new(
        device: &wgpu::Device,
        bind_group_layouts: &HashMap<BindGroupLayoutType, wgpu::BindGroupLayout>,
    ) -> Shadow {
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Shadow Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        let shadow_texture = device.create_texture(&wgpu::TextureDescriptor {
            size: SHADOW_SIZE,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SHADOW_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            label: None,
            view_formats: &[],
        });
        let shadow_view = shadow_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let shadow_target_views = (0..2)
            .map(|i| {
                shadow_texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("shadow"),
                    format: None,
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    aspect: wgpu::TextureAspect::All,
                    base_mip_level: 0,
                    mip_level_count: None,
                    base_array_layer: i as u32,
                    array_layer_count: NonZeroU32::new(1),
                })
            })
            .collect::<Vec<_>>();

        let shadow_pipeline = {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("shadow"),
                bind_group_layouts: &[&bind_group_layouts
                    .get(&BindGroupLayoutType::Light)
                    .unwrap()],
                push_constant_ranges: &[],
            });

            let shadow_shader_desc = wgpu::ShaderModuleDescriptor {
                label: Some("Shadow bake shader"),
                source: wgpu::ShaderSource::Wgsl(
                    include_str!("shaders/shadow_bake_vert.wgsl").into(),
                ),
            };

            let shadow_shader = device.create_shader_module(shadow_shader_desc);

            // Create the render pipeline
            let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("shadow"),
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
                    format: SHADOW_FORMAT,
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
            layout: &bind_group_layouts
                .get(&BindGroupLayoutType::DepthTexture)
                .unwrap(),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&shadow_sampler),
                },
            ],
            label: None,
        });

        Shadow {
            bind_group,
            shadow_pipeline,
            shadow_target_views,
        }
    }

    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        model: &Model,
        light_bind_group: &BindGroup,
        instances: usize,
        instance_buffer: &Buffer,
    ) {
        let mut shadow_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Shadow pass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: &self.shadow_target_views[0],
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: true,
                }),
                stencil_ops: None,
            }),
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
