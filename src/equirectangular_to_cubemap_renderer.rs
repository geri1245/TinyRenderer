use std::rc::Rc;

use wgpu::TextureFormat;

use crate::{model::RenderableMesh, pipelines::EquirectangularToCubemapRP};

pub struct EquirectangularToCubemapRenderer {
    pipeline: EquirectangularToCubemapRP,
    mesh: Rc<RenderableMesh>,
}

impl EquirectangularToCubemapRenderer {
    pub async fn new(
        device: &wgpu::Device,
        color_format: TextureFormat,
        basic_mesh: Rc<RenderableMesh>,
    ) -> anyhow::Result<Self> {
        let pipeline = EquirectangularToCubemapRP::new(device, color_format).await?;

        Ok(Self {
            pipeline,
            mesh: basic_mesh,
        })
    }
}
