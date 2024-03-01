use wgpu::{ComputePipeline, Device, PipelineLayout, ShaderModule};

use crate::{
    bind_group_layout_descriptors, camera_controller::CameraController,
    light_controller::LightController,
};

use super::{
    render_pipeline_base::PipelineBase,
    shader_compilation_result::{CompiledShader, PipelineRecreationResult},
};

const SHADER_SOURCE: &'static str = "src/shaders/main.wgsl";

pub struct MainRP {
    compute_pipeline: ComputePipeline,
    shader_modification_time: u64,
}

impl PipelineBase for MainRP {}

impl MainRP {
    fn create_render_pipeline(
        device: &wgpu::Device,
        shader: &ShaderModule,
        render_pipeline_layout: &PipelineLayout,
    ) -> ComputePipeline {
        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Compute pipeline that does the lighting from the gbuffer"),
            layout: Some(render_pipeline_layout),
            entry_point: "cs_main",
            module: shader,
        })
    }

    fn create_pipeline_layout(device: &Device) -> PipelineLayout {
        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Main Render Pipeline Layout"),
            bind_group_layouts: &[
                &device.create_bind_group_layout(&bind_group_layout_descriptors::LIGHT),
                &device.create_bind_group_layout(&bind_group_layout_descriptors::CAMERA),
                &device.create_bind_group_layout(&bind_group_layout_descriptors::GBUFFER),
                &device
                    .create_bind_group_layout(&bind_group_layout_descriptors::SHADOW_DEPTH_TEXTURE),
                &device.create_bind_group_layout(&bind_group_layout_descriptors::COMPUTE_PING_PONG),
            ],
            push_constant_ranges: &[],
        })
    }

    pub async fn new(device: &Device) -> anyhow::Result<Self> {
        let shader = Self::compile_shader_if_needed(SHADER_SOURCE, device).await?;
        Result::Ok(Self::new_internal(&shader, device))
    }

    fn new_internal(shader: &CompiledShader, device: &Device) -> Self {
        let render_pipeline_layout = Self::create_pipeline_layout(device);

        let compute_pipeline =
            Self::create_render_pipeline(device, &shader.shader_module, &render_pipeline_layout);

        Self {
            compute_pipeline,
            shader_modification_time: shader.last_write_time,
        }
    }

    pub async fn try_recompile_shader(&self, device: &Device) -> PipelineRecreationResult<Self> {
        if !Self::need_recompile_shader(SHADER_SOURCE, self.shader_modification_time).await {
            return PipelineRecreationResult::AlreadyUpToDate;
        }

        match Self::compile_shader_if_needed(SHADER_SOURCE, device).await {
            Ok(compiled_shader) => {
                PipelineRecreationResult::Success(Self::new_internal(&compiled_shader, device))
            }
            Err(error) => PipelineRecreationResult::Failed(error),
        }
    }

    pub fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::ComputePass<'a>,
        camera_controller: &'a CameraController,
        light_controller: &'a LightController,
        gbuffer_bind_group: &'a wgpu::BindGroup,
        shadow_bind_group: &'a wgpu::BindGroup,
        copmute_pass_textures_bind_group: &'a wgpu::BindGroup,
        width: u32,
        height: u32,
    ) {
        render_pass.set_pipeline(&self.compute_pipeline);

        render_pass.set_bind_group(1, &camera_controller.bind_group, &[]);
        render_pass.set_bind_group(0, &light_controller.light_bind_group, &[]);
        render_pass.set_bind_group(2, gbuffer_bind_group, &[]);
        render_pass.set_bind_group(3, shadow_bind_group, &[]);
        render_pass.set_bind_group(4, copmute_pass_textures_bind_group, &[]);

        render_pass.dispatch_workgroups(width, height, 1);
    }
}
