use wgpu::{
    BindGroup, BindGroupLayout, BindGroupLayoutDescriptor, ComputePass, ComputePipeline, Device,
    PipelineCompilationOptions, ShaderModule,
};

use super::shader_compiler::{ShaderCompilationResult, ShaderCompilationSuccess, ShaderCompiler};

pub struct SimpleCP {
    pipeline: wgpu::ComputePipeline,
    shader_compiler: ShaderCompiler,
    label: String,
    bind_group_layouts: Vec<BindGroupLayout>,
}

impl SimpleCP {
    pub async fn new<'a>(
        device: &wgpu::Device,
        bind_group_layout_descriptors: &[&BindGroupLayoutDescriptor<'static>],
        shader_source: &'static str,
        label: &str,
    ) -> anyhow::Result<Self> {
        let mut shader_compiler = ShaderCompiler::new(shader_source.to_string());
        let shader_compilation_result = shader_compiler.compile_shader_if_needed(device).await?;
        let label = label.to_owned();
        let bind_group_layouts = bind_group_layout_descriptors
            .iter()
            .map(|desc| device.create_bind_group_layout(desc))
            .collect::<Vec<_>>();

        let mut bind_group_layout_refs = Vec::new();

        for desc in &bind_group_layouts {
            bind_group_layout_refs.push(desc);
        }

        match shader_compilation_result {
            ShaderCompilationResult::AlreadyUpToDate => {
                panic!("This shader hasn't been compiled yet, can't be up to date!")
            }
            ShaderCompilationResult::Success(shader) => Ok(Self {
                pipeline: Self::create_pipeline(device, &shader, &bind_group_layout_refs, &label),
                shader_compiler,
                label,
                bind_group_layouts,
            }),
        }
    }

    pub async fn try_recompile_shader<'a>(
        &'a mut self,
        device: &'a Device,
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
                let mut bind_group_layout_refs = Vec::new();

                for bind_group_layout in &self.bind_group_layouts {
                    bind_group_layout_refs.push(bind_group_layout);
                }

                let pipeline = Self::create_pipeline(
                    device,
                    &shader_module,
                    &bind_group_layout_refs,
                    &self.label,
                );
                self.pipeline = pipeline;
                Ok(ShaderCompilationSuccess::Recompiled)
            }
        }
    }

    fn create_pipeline<'a>(
        device: &Device,
        shader: &ShaderModule,
        bind_group_layout_descriptors: &[&BindGroupLayout],
        label: &String,
    ) -> ComputePipeline {
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(&format!("{label} pipeline layout")),
            bind_group_layouts: bind_group_layout_descriptors,
            push_constant_ranges: &[],
        });

        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            compilation_options: PipelineCompilationOptions::default(),
            label: Some(&format!("{label} pipeline")),
            module: shader,
            entry_point: "cs_main",
            layout: Some(&pipeline_layout),
            cache: None,
        })
    }

    pub fn run_copmute_pass<'a>(
        &'a self,
        compute_pass: &mut ComputePass<'a>,
        bind_groups: &[&'a BindGroup],
        workgroup_dimensions: (u32, u32, u32),
    ) {
        compute_pass.set_pipeline(&self.pipeline);

        for (index, bind_group) in bind_groups.iter().enumerate() {
            compute_pass.set_bind_group(index as u32, bind_group, &[]);
        }

        compute_pass.dispatch_workgroups(
            workgroup_dimensions.0,
            workgroup_dimensions.1,
            workgroup_dimensions.2,
        );
    }
}
