use std::rc::Rc;

use async_std::task::block_on;
use wgpu::{
    CommandEncoder, Device, ImageCopyTexture, ImageDataLayout, SubmissionIndex, TextureAspect,
    TextureFormat,
};

use crate::{
    bind_group_layout_descriptors,
    buffer_capture::OutputBuffer,
    cubemap_helpers::{create_cubemap_face_rendering_parameters, RenderingIntoCubemapResources},
    model::RenderableMesh,
    pipelines::{DiffuseIrradianceBakerRP, ShaderCompilationSuccess},
};

const CUBEMAP_RESOLUTION: u32 = 64;
const IBL_MAP_EXTENT: wgpu::Extent3d = wgpu::Extent3d {
    width: CUBEMAP_RESOLUTION,
    height: CUBEMAP_RESOLUTION,
    depth_or_array_layers: 6,
};

const DEFAULT_IBL: &[u8] = include_bytes!("../assets/textures/defaults/irradiance_map.data");

pub struct DiffuseIrradianceRenderer {
    pipeline: DiffuseIrradianceBakerRP,
    mesh: Rc<RenderableMesh>,
    cube_face_rendering_params: Vec<RenderingIntoCubemapResources>,
    pub diffuse_irradiance_cubemap: Rc<wgpu::BindGroup>,
    color_format: wgpu::TextureFormat,
    output_buffer: OutputBuffer,
    ibl_irradiance_texture: wgpu::Texture,
}

impl DiffuseIrradianceRenderer {
    pub async fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: TextureFormat,
        basic_mesh: Rc<RenderableMesh>,
    ) -> anyhow::Result<Self> {
        let pipeline = DiffuseIrradianceBakerRP::new(device, color_format).await?;

        let texture_descriptor = wgpu::TextureDescriptor {
            size: IBL_MAP_EXTENT,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: color_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            label: None,
            view_formats: &[],
        };

        let ibl_irradiance_texture = device.create_texture(&texture_descriptor);
        queue.write_texture(
            ibl_irradiance_texture.as_image_copy(),
            DEFAULT_IBL,
            ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(IBL_MAP_EXTENT.width * 4 * 2), // 4 components, 2 bytes per component (rgba16)
                rows_per_image: Some(IBL_MAP_EXTENT.height),
            },
            IBL_MAP_EXTENT,
        );

        let sampled_cube_view = ibl_irradiance_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Diffuse irradiance cube tartget view"),
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });

        let cube_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Diffuse irradiance cube map sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let sampled_cubemap_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &device.create_bind_group_layout(
                &bind_group_layout_descriptors::TEXTURE_CUBE_FRAGMENT_COMPUTE_WITH_SAMPLER,
            ),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&sampled_cube_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&cube_sampler),
                },
            ],
            label: None,
        });

        let render_into_cubemap_params =
            create_cubemap_face_rendering_parameters(device, &ibl_irradiance_texture);

        let output_buffer = OutputBuffer::new(device, &IBL_MAP_EXTENT, &color_format);

        Ok(Self {
            pipeline,
            mesh: basic_mesh,
            cube_face_rendering_params: render_into_cubemap_params,
            diffuse_irradiance_cubemap: Rc::new(sampled_cubemap_bind_group),
            color_format,
            output_buffer,
            ibl_irradiance_texture,
        })
    }

    pub async fn try_recompile_shader(
        &mut self,
        device: &Device,
    ) -> anyhow::Result<ShaderCompilationSuccess> {
        self.pipeline
            .try_recompile_shader(device, self.color_format)
            .await
    }

    pub fn render(&self, encoder: &mut CommandEncoder, hdr_environment_cube_map: &wgpu::BindGroup) {
        for render_param in &self.cube_face_rendering_params {
            self.pipeline.render(
                encoder,
                &render_param.cube_target_texture_view,
                &self.mesh,
                &render_param.cube_face_viewproj_bind_group,
                hdr_environment_cube_map,
            );
        }

        encoder.copy_texture_to_buffer(
            ImageCopyTexture {
                aspect: TextureAspect::All,
                texture: &self.ibl_irradiance_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            wgpu::ImageCopyBuffer {
                buffer: &self.output_buffer.buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(self.output_buffer.padded_row_size as u32),
                    rows_per_image: Some(IBL_MAP_EXTENT.height),
                },
            },
            self.output_buffer.texture_extent,
        );
    }

    pub fn write_current_ibl_to_file(
        &self,
        device: &Device,
        submission_index: Option<SubmissionIndex>,
    ) {
        block_on(self.output_buffer.save_buffer_to_file(
            "output_ibl.data",
            submission_index,
            device,
        ))
    }
}
