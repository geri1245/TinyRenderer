use crate::{
    bind_group_layout_descriptors, camera_controller::CameraController,
    light_controller::LightController, texture,
};

pub struct MainRP {
    render_pipeline: wgpu::RenderPipeline,
}

impl MainRP {
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        let shader_desc = wgpu::ShaderModuleDescriptor {
            label: Some("GBuffer processing shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/main.wgsl").into()),
        };

        let shader = device.create_shader_module(shader_desc);

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Main Render Pipeline Layout"),
                bind_group_layouts: &[
                    &device.create_bind_group_layout(&bind_group_layout_descriptors::LIGHT),
                    &device.create_bind_group_layout(&bind_group_layout_descriptors::CAMERA),
                    &device.create_bind_group_layout(&bind_group_layout_descriptors::GBUFFER),
                    &device.create_bind_group_layout(&bind_group_layout_descriptors::DEPTH_TEXTURE),
                ],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Main render pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
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
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        Self { render_pipeline }
    }

    pub fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_controller: &'a CameraController,
        light_controller: &'a LightController,
        gbuffer_bind_group: &'a wgpu::BindGroup,
        shadow_bind_group: &'a wgpu::BindGroup,
    ) {
        render_pass.set_pipeline(&self.render_pipeline);

        render_pass.set_bind_group(1, &camera_controller.bind_group, &[]);
        render_pass.set_bind_group(0, &light_controller.bind_group, &[]);
        render_pass.set_bind_group(2, gbuffer_bind_group, &[]);
        render_pass.set_bind_group(3, shadow_bind_group, &[]);

        render_pass.draw(0..3, 0..1);
    }
}
