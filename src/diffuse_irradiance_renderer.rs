use wgpu::Device;

use crate::{bind_group_layout_descriptors, pipelines::SimpleCP};

const POST_PROCESS_SHADER_SOURCE: &'static str = "src/shaders/diffuse_irradiance_bake.wgsl";

pub struct DiffuseIrradianceRenderer {
    copmute_pipeline: SimpleCP,
}

impl DiffuseIrradianceRenderer {
    pub async fn new(device: &Device) -> Self {
        let pipeline = SimpleCP::new(
            device,
            &bind_group_layout_descriptors::COMPUTE_FINAL_STAGE,
            POST_PROCESS_SHADER_SOURCE,
        )
        .await
        .unwrap();

        Self {
            copmute_pipeline: pipeline,
        }
    }
}
