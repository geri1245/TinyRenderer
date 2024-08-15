use wgpu::{
    BindGroup, BindGroupLayoutDescriptor, ComputePass, ComputePipeline, Device,
    PipelineCompilationOptions, ShaderModule,
};

use super::{
    shader_compiler::{ShaderCompilationResult, ShaderCompiler},
    ShaderCompilationSuccess,
};

pub struct SimpleCP {
    pipeline: wgpu::ComputePipeline,
    shader_compiler: ShaderCompiler,
}

impl SimpleCP {
    pub async fn new<'a>(
        device: &wgpu::Device,
        bind_group_layout_descriptor: &'a BindGroupLayoutDescriptor<'a>,
        shader_source: &'static str,
    ) -> anyhow::Result<Self> {
        let mut shader_compiler = ShaderCompiler::new(shader_source);
        let shader_compilation_result = shader_compiler.compile_shader_if_needed(device).await?;

        match shader_compilation_result {
            ShaderCompilationResult::AlreadyUpToDate => {
                panic!("This shader hasn't been compiled yet, can't be up to date!")
            }
            ShaderCompilationResult::Success(shader) => Ok(Self {
                pipeline: Self::create_pipeline(device, &shader, bind_group_layout_descriptor),
                shader_compiler,
            }),
        }
    }

    pub async fn try_recompile_shader<'a>(
        &'a mut self,
        device: &'a Device,
        bind_group_layout_descriptor: &BindGroupLayoutDescriptor<'a>,
    ) -> anyhow::Result<ShaderCompilationSuccess> {
        let result = self
            .shader_compiler
            .compile_shader_if_needed(device)
            .await?;

        match result {
            ShaderCompilationResult::AlreadyUpToDate => {
                Ok(ShaderCompilationSuccess::AlreadyUpToDate)
            }
            ShaderCompilationResult::Success(shader_module) => {
                let pipeline =
                    Self::create_pipeline(device, &shader_module, bind_group_layout_descriptor);
                self.pipeline = pipeline;
                Ok(ShaderCompilationSuccess::Recompiled)
            }
        }
    }

    fn create_pipeline<'a>(
        device: &Device,
        shader: &ShaderModule,
        bind_group_layout_descriptor: &BindGroupLayoutDescriptor<'a>,
    ) -> ComputePipeline {
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Main Render Pipeline Layout"),
            bind_group_layouts: &[&device.create_bind_group_layout(bind_group_layout_descriptor)],
            push_constant_ranges: &[],
        });

        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            compilation_options: PipelineCompilationOptions::default(),
            label: Some("Compute pipeline for posteffects"),
            module: shader,
            entry_point: "cs_main",
            layout: Some(&pipeline_layout),
        })
    }

    pub fn run_copmute_pass<'a>(
        &'a self,
        compute_pass: &mut ComputePass<'a>,
        compute_pass_texture_bind_group: &'a BindGroup,
        workgroup_dimensions: (u32, u32, u32),
    ) {
        compute_pass.set_pipeline(&self.pipeline);
        compute_pass.set_bind_group(0, &compute_pass_texture_bind_group, &[]);

        compute_pass.dispatch_workgroups(
            workgroup_dimensions.0,
            workgroup_dimensions.1,
            workgroup_dimensions.2,
        );
    }
}
