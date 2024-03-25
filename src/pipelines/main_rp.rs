use wgpu::{ComputePipeline, Device, ShaderModule};

use crate::{
    bind_group_layout_descriptors, camera_controller::CameraController,
    light_controller::LightController,
};

use super::shader_compiler::{ShaderCompilationResult, ShaderCompiler};

const SHADER_SOURCE: &'static str = "src/shaders/main.wgsl";

pub struct MainRP {
    compute_pipeline: ComputePipeline,
    shader_compiler: ShaderCompiler,
}

impl MainRP {
    pub async fn new(device: &Device) -> anyhow::Result<Self> {
        let mut shader_compiler = ShaderCompiler::new(SHADER_SOURCE);
        let shader_compilation_result = shader_compiler.compile_shader_if_needed(device).await?;

        match shader_compilation_result {
            ShaderCompilationResult::AlreadyUpToDate => {
                panic!("This shader hasn't been compiled yet, can't be up to date!")
            }
            ShaderCompilationResult::Success(shader) => Ok(Self {
                compute_pipeline: Self::create_pipeline(device, &shader),
                shader_compiler,
            }),
        }
    }

    fn create_pipeline(device: &Device, shader: &ShaderModule) -> ComputePipeline {
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Main Render Pipeline Layout"),
                bind_group_layouts: &[
                    &device.create_bind_group_layout(&bind_group_layout_descriptors::LIGHT),
                    &device.create_bind_group_layout(
                        &bind_group_layout_descriptors::BUFFER_VISIBLE_EVERYWHERE,
                    ),
                    &device.create_bind_group_layout(&bind_group_layout_descriptors::GBUFFER),
                    &device.create_bind_group_layout(
                        &bind_group_layout_descriptors::SHADOW_DEPTH_TEXTURE,
                    ),
                    &device.create_bind_group_layout(
                        &bind_group_layout_descriptors::COMPUTE_PING_PONG,
                    ),
                ],
                push_constant_ranges: &[],
            });

        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Compute pipeline that does the lighting from the gbuffer"),
            layout: Some(&render_pipeline_layout),
            entry_point: "cs_main",
            module: shader,
        })
    }

    pub async fn try_recompile_shader(&mut self, device: &Device) -> anyhow::Result<()> {
        let result = self
            .shader_compiler
            .compile_shader_if_needed(device)
            .await?;

        match result {
            ShaderCompilationResult::AlreadyUpToDate => Ok(()),
            ShaderCompilationResult::Success(shader_module) => {
                let pipeline = Self::create_pipeline(device, &shader_module);
                self.compute_pipeline = pipeline;
                Ok(())
            }
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
