use wgpu::{BindGroup, ComputePass, Device, PipelineLayout, ShaderModule};

use crate::bind_group_layout_descriptors;

use super::render_pipeline_base::PipelineBase;

const SHADER_SOURCE: &'static str = "src/shaders/post_process.wgsl";

pub struct PostProcessRP {
    pipeline: wgpu::ComputePipeline,
    shader_compilation_time: u64,
}

impl PipelineBase for PostProcessRP {}

impl PostProcessRP {
    pub async fn new(device: &wgpu::Device) -> anyhow::Result<Self> {
        let shader = Self::compile_shader_if_needed(SHADER_SOURCE, device).await?;

        Ok(Self::new_internal(
            device,
            &shader.shader_module,
            shader.last_write_time,
        ))
    }

    fn create_pipeline_layout(device: &Device) -> PipelineLayout {
        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Main Render Pipeline Layout"),
            bind_group_layouts: &[
                &device.create_bind_group_layout(&bind_group_layout_descriptors::COMPUTE_PING_PONG)
            ],
            push_constant_ranges: &[],
        })
    }

    fn new_internal(device: &Device, shader: &ShaderModule, shader_compilation_time: u64) -> Self {
        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Compute pipeline for posteffects"),
            module: shader,
            entry_point: "cs_main",
            layout: Some(&Self::create_pipeline_layout(device)),
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
        width: u32,
        height: u32,
    ) {
        compute_pass.set_pipeline(&self.pipeline);
        compute_pass.set_bind_group(0, &compute_pass_texture_bind_groups, &[]);

        compute_pass.dispatch_workgroups(width, height, 1);
    }
}
