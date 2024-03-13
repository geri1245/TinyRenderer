use wgpu::{BindGroup, ComputePass, Device, PipelineLayout, ShaderModule};

use crate::bind_group_layout_descriptors;

use super::render_pipeline_base::PipelineBase;

const SHADER_SOURCE: &'static str = "src/shaders/post_process.wgsl";

pub enum PostProcessPipelineTargetTextureVariant {
    _Rgba16Float,
    Rgba8Unorm,
}

pub struct PostProcessRP {
    pipeline: wgpu::ComputePipeline,
    shader_compilation_time: u64,
}

impl PipelineBase for PostProcessRP {}

impl PostProcessRP {
    pub async fn new(
        device: &wgpu::Device,
        variant: PostProcessPipelineTargetTextureVariant,
    ) -> anyhow::Result<Self> {
        let shader = Self::compile_shader_if_needed(SHADER_SOURCE, device).await?;

        Ok(Self::new_internal(
            device,
            variant,
            &shader.shader_module,
            shader.last_write_time,
        ))
    }

    fn create_pipeline_layout(
        device: &Device,
        variant: PostProcessPipelineTargetTextureVariant,
    ) -> PipelineLayout {
        let bind_group_descriptor = match variant {
            PostProcessPipelineTargetTextureVariant::_Rgba16Float => {
                &bind_group_layout_descriptors::COMPUTE_PING_PONG
            }
            PostProcessPipelineTargetTextureVariant::Rgba8Unorm => {
                &bind_group_layout_descriptors::COMPUTE_FINAL_STAGE
            }
        };
        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Main Render Pipeline Layout"),
            bind_group_layouts: &[&device.create_bind_group_layout(bind_group_descriptor)],
            push_constant_ranges: &[],
        })
    }

    fn new_internal(
        device: &Device,
        variant: PostProcessPipelineTargetTextureVariant,
        shader: &ShaderModule,
        shader_compilation_time: u64,
    ) -> Self {
        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Compute pipeline for posteffects"),
            module: shader,
            entry_point: "cs_main",
            layout: Some(&Self::create_pipeline_layout(device, variant)),
        });

        Self {
            pipeline: compute_pipeline,
            shader_compilation_time,
        }
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
