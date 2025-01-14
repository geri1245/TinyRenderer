use wgpu::{ComputePipeline, Device, PipelineCompilationOptions, ShaderModule};

use crate::{
    bind_group_layout_descriptors, camera_controller::CameraController,
    light_controller::LightController,
};

use super::shader_compiler::{ShaderCompilationResult, ShaderCompilationSuccess, ShaderCompiler};

const SHADER_SOURCE: &'static str = "src/shaders/main.wgsl";
const WORKGROUP_SIZE_PER_DIMENSION: u32 = 8;

pub struct MainRP {
    compute_pipeline: ComputePipeline,
    shader_compiler: ShaderCompiler,
}

impl MainRP {
    pub fn new(device: &Device) -> anyhow::Result<Self> {
        let mut shader_compiler = ShaderCompiler::new(SHADER_SOURCE.to_string());
        let shader_compilation_result = shader_compiler.compile_shader_if_needed(device)?;

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
                        &bind_group_layout_descriptors::DEPTH_TEXTURE_ARRAY,
                    ),
                    &device.create_bind_group_layout(
                        &bind_group_layout_descriptors::DEPTH_TEXTURE_CUBE_ARRAY,
                    ),
                    &device.create_bind_group_layout(
                        &bind_group_layout_descriptors::COMPUTE_PING_PONG,
                    ),
                    &device.create_bind_group_layout(
                        &bind_group_layout_descriptors::TEXTURE_CUBE_FRAGMENT_COMPUTE_WITH_SAMPLER,
                    ),
                    &device.create_bind_group_layout(
                        &bind_group_layout_descriptors::BUFFER_VISIBLE_EVERYWHERE,
                    ),
                ],
                push_constant_ranges: &[],
            });

        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            compilation_options: PipelineCompilationOptions::default(),
            label: Some("Compute pipeline that does the lighting from the gbuffer"),
            layout: Some(&render_pipeline_layout),
            entry_point: Some("cs_main"),
            module: shader,
            cache: None,
        })
    }

    pub fn try_recompile_shader(
        &mut self,
        device: &Device,
    ) -> anyhow::Result<ShaderCompilationSuccess> {
        let result = self.shader_compiler.compile_shader_if_needed(device)?;

        match result {
            ShaderCompilationResult::AlreadyUpToDate => {
                Ok(ShaderCompilationSuccess::AlreadyUpToDate)
            }
            ShaderCompilationResult::Success(shader_module) => {
                let pipeline = Self::create_pipeline(device, &shader_module);
                self.compute_pipeline = pipeline;
                Ok(ShaderCompilationSuccess::Recompiled)
            }
        }
    }

    pub fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::ComputePass<'a>,
        camera_controller: &'a CameraController,
        light_controller: &'a LightController,
        gbuffer_bind_group: &'a wgpu::BindGroup,
        directional_lights_depth_texture_bg: &'a wgpu::BindGroup,
        point_lights_depth_texture_bg: &'a wgpu::BindGroup,
        diffuse_irradiance_map_bind_group: &'a wgpu::BindGroup,
        copmute_pass_textures_bind_group: &'a wgpu::BindGroup,
        render_target_width: u32,
        render_target_height: u32,
    ) {
        render_pass.set_pipeline(&self.compute_pipeline);
        render_pass.set_bind_group(0, light_controller.get_light_bind_group(), &[]);
        render_pass.set_bind_group(1, &camera_controller.bind_group, &[]);
        render_pass.set_bind_group(2, gbuffer_bind_group, &[]);
        render_pass.set_bind_group(3, directional_lights_depth_texture_bg, &[]);
        render_pass.set_bind_group(4, point_lights_depth_texture_bg, &[]);
        render_pass.set_bind_group(5, copmute_pass_textures_bind_group, &[]);
        render_pass.set_bind_group(6, diffuse_irradiance_map_bind_group, &[]);
        render_pass.set_bind_group(7, light_controller.get_light_parameters_bind_group(), &[]);

        let num_dispatches_x = render_target_width.div_ceil(WORKGROUP_SIZE_PER_DIMENSION);
        let num_dispatches_y = render_target_height.div_ceil(WORKGROUP_SIZE_PER_DIMENSION);

        render_pass.dispatch_workgroups(num_dispatches_x, num_dispatches_y, 1);
    }
}
