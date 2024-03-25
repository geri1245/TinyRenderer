use wgpu::{BindGroup, ComputePass, ComputePipeline, Device, ShaderModule};

use crate::bind_group_layout_descriptors;

use super::shader_compiler::{ShaderCompilationResult, ShaderCompiler};

const SHADER_SOURCE: &'static str = "src/shaders/post_process.wgsl";

#[derive(Clone, Copy)]
pub enum PostProcessPipelineTargetTextureVariant {
    _Rgba16Float,
    Rgba8Unorm,
}

pub struct PostProcessRP {
    pipeline: wgpu::ComputePipeline,
    shader_compiler: ShaderCompiler,
}

impl PostProcessRP {
    pub async fn new(
        device: &wgpu::Device,
        variant: PostProcessPipelineTargetTextureVariant,
    ) -> anyhow::Result<Self> {
        let mut shader_compiler = ShaderCompiler::new(SHADER_SOURCE);
        let shader_compilation_result = shader_compiler.compile_shader_if_needed(device).await?;

        match shader_compilation_result {
            ShaderCompilationResult::AlreadyUpToDate => {
                panic!("This shader hasn't been compiled yet, can't be up to date!")
            }
            ShaderCompilationResult::Success(shader) => Ok(Self {
                pipeline: Self::create_pipeline(device, &shader, variant),
                shader_compiler,
            }),
        }
    }

    pub async fn try_recompile_shader(
        &mut self,
        device: &Device,
        variant: PostProcessPipelineTargetTextureVariant,
    ) -> anyhow::Result<()> {
        let result = self
            .shader_compiler
            .compile_shader_if_needed(device)
            .await?;

        match result {
            ShaderCompilationResult::AlreadyUpToDate => Ok(()),
            ShaderCompilationResult::Success(shader_module) => {
                let pipeline = Self::create_pipeline(device, &shader_module, variant);
                self.pipeline = pipeline;
                Ok(())
            }
        }
    }

    fn create_pipeline(
        device: &Device,
        shader: &ShaderModule,
        variant: PostProcessPipelineTargetTextureVariant,
    ) -> ComputePipeline {
        let bind_group_descriptor = match variant {
            PostProcessPipelineTargetTextureVariant::_Rgba16Float => {
                &bind_group_layout_descriptors::COMPUTE_PING_PONG
            }
            PostProcessPipelineTargetTextureVariant::Rgba8Unorm => {
                &bind_group_layout_descriptors::COMPUTE_FINAL_STAGE
            }
        };
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Main Render Pipeline Layout"),
            bind_group_layouts: &[&device.create_bind_group_layout(bind_group_descriptor)],
            push_constant_ranges: &[],
        });

        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Compute pipeline for posteffects"),
            module: shader,
            entry_point: "cs_main",
            layout: Some(&pipeline_layout),
        })
    }

    pub fn run_copmute_pass<'a>(
        &'a self,
        compute_pass: &mut ComputePass<'a>,
        compute_pass_texture_bind_groups: &'a BindGroup,
        workgroup_dimensions: (u32, u32, u32),
    ) {
        compute_pass.set_pipeline(&self.pipeline);
        compute_pass.set_bind_group(0, &compute_pass_texture_bind_groups, &[]);

        compute_pass.dispatch_workgroups(
            workgroup_dimensions.0,
            workgroup_dimensions.1,
            workgroup_dimensions.2,
        );
    }
}
