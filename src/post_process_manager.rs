use wgpu::{BindGroup, ComputePass, Device};

use crate::pipelines::PostProcessRP;

const WORKGROUP_SIZE_PER_DIMENSION: u32 = 8;

pub struct PostProcessManager {
    pub pipeline: PostProcessRP,
}

impl PostProcessManager {
    pub async fn new(device: &Device) -> Self {
        let pipeline = PostProcessRP::new(device).await.unwrap();

        Self { pipeline }
    }

    pub fn render<'a>(
        &'a self,
        compute_pass: &mut ComputePass<'a>,
        compute_pass_texture_bind_groups: &'a BindGroup,
        render_target_width: u32,
        render_target_height: u32,
    ) {
        let num_dispatches_x = render_target_width.div_ceil(WORKGROUP_SIZE_PER_DIMENSION);
        let num_dispatches_y = render_target_height.div_ceil(WORKGROUP_SIZE_PER_DIMENSION);
        self.pipeline.run_copmute_pass(
            compute_pass,
            compute_pass_texture_bind_groups,
            (num_dispatches_x, num_dispatches_y, 1),
        );
    }
}
