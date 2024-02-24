use wgpu::{Device, PipelineLayout, RenderPipeline, ShaderModule, TextureFormat};

use crate::{
    bind_group_layout_descriptors, camera_controller::CameraController,
    light_controller::LightController, texture,
};

use super::{
    render_pipeline_base::RenderPipelineBase,
    shader_compilation_result::{CompiledShader, PipelineRecreationResult},
};

const SHADER_SOURCE: &'static str = "src/shaders/main.wgsl";

pub struct MainRP {
    render_pipeline: RenderPipeline,
    shader_modification_time: u64,
    color_format: wgpu::TextureFormat,
}

impl RenderPipelineBase for MainRP {}

impl MainRP {
    fn create_render_pipeline(
        device: &wgpu::Device,
        shader: &ShaderModule,
        color_format: wgpu::TextureFormat,
        render_pipeline_layout: &PipelineLayout,
    ) -> RenderPipeline {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Main render pipeline"),
            layout: Some(render_pipeline_layout),
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
                format: texture::SampledTexture::DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        })
    }

    fn create_pipeline_layout(device: &Device) -> PipelineLayout {
        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Main Render Pipeline Layout"),
            bind_group_layouts: &[
                &device.create_bind_group_layout(&bind_group_layout_descriptors::LIGHT),
                &device.create_bind_group_layout(&bind_group_layout_descriptors::CAMERA),
                &device.create_bind_group_layout(&bind_group_layout_descriptors::GBUFFER),
                &device.create_bind_group_layout(&bind_group_layout_descriptors::DEPTH_TEXTURE),
            ],
            push_constant_ranges: &[],
        })
    }

    pub async fn new(device: &Device, color_format: TextureFormat) -> anyhow::Result<Self> {
        let shader = Self::compile_shader_if_needed(SHADER_SOURCE, device).await?;
        Result::Ok(Self::new_internal(&shader, device, color_format))
    }

    fn new_internal(shader: &CompiledShader, device: &Device, color_format: TextureFormat) -> Self {
        let render_pipeline_layout = Self::create_pipeline_layout(device);

        let render_pipeline = Self::create_render_pipeline(
            device,
            &shader.shader_module,
            color_format,
            &render_pipeline_layout,
        );

        Self {
            render_pipeline,
            shader_modification_time: shader.last_write_time,
            color_format,
        }
    }

    pub async fn try_recompile_shader(&self, device: &Device) -> PipelineRecreationResult<Self> {
        if !Self::need_recompile_shader(SHADER_SOURCE, self.shader_modification_time).await {
            return PipelineRecreationResult::AlreadyUpToDate;
        }

        match Self::compile_shader_if_needed(SHADER_SOURCE, device).await {
            Ok(compiled_shader) => PipelineRecreationResult::Success(Self::new_internal(
                &compiled_shader,
                device,
                self.color_format,
            )),
            Err(error) => PipelineRecreationResult::Failed(error),
        }
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
        render_pass.set_bind_group(0, &light_controller.light_bind_group, &[]);
        render_pass.set_bind_group(2, gbuffer_bind_group, &[]);
        render_pass.set_bind_group(3, shadow_bind_group, &[]);

        render_pass.draw(0..3, 0..1);
    }
}
