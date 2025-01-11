use std::rc::Rc;

use wgpu::{CommandEncoder, Device, TextureFormat};

use crate::{
    bind_group_layout_descriptors,
    cubemap_helpers::{create_cubemap_face_rendering_parameters, RenderingIntoCubemapResources},
    model::Primitive,
    pipelines::{EquirectangularToCubemapRP, ShaderCompilationSuccess},
    texture::SampledTexture,
};

const CUBEMAP_RESOLUTION: u32 = 1024;

pub struct EquirectangularToCubemapRenderer {
    pipeline: EquirectangularToCubemapRP,
    mesh: Rc<Primitive>,
    hdr_map_bind_group: wgpu::BindGroup,
    render_params: Vec<RenderingIntoCubemapResources>,
    pub cube_map_to_sample: Rc<wgpu::BindGroup>,
    color_format: wgpu::TextureFormat,
}

impl EquirectangularToCubemapRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: TextureFormat,
        basic_mesh: Rc<Primitive>,
    ) -> anyhow::Result<Self> {
        let pipeline = EquirectangularToCubemapRP::new(device, color_format)?;
        let hdr_texture_path = "assets/textures/skybox/golf_course.hdr";
        let hdr_texture = SampledTexture::from_hdr_image(
            device,
            queue,
            hdr_texture_path,
            Some("HDR equirectangular map"),
        )
        .unwrap();

        let size = wgpu::Extent3d {
            width: CUBEMAP_RESOLUTION,
            height: CUBEMAP_RESOLUTION,
            depth_or_array_layers: 6,
        };

        let texture_descriptor = wgpu::TextureDescriptor {
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: color_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            label: None,
            view_formats: &[],
        };

        let cube_texture = device.create_texture(&texture_descriptor);

        let sampled_cube_view = cube_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("HDR cubemap target view"),
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });

        let cube_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Equirectangular cube map sampler"),
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

        let hdr_map_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &device.create_bind_group_layout(
                &bind_group_layout_descriptors::TEXTURE_2D_FRAGMENT_WITH_SAMPLER,
            ),
            entries: &[
                hdr_texture.get_texture_bind_group_entry(0),
                hdr_texture.get_sampler_bind_group_entry(1),
            ],
            label: None,
        });

        let render_params = create_cubemap_face_rendering_parameters(device, &cube_texture);

        Ok(Self {
            pipeline,
            mesh: basic_mesh,
            hdr_map_bind_group,
            render_params,
            cube_map_to_sample: Rc::new(sampled_cubemap_bind_group),
            color_format,
        })
    }

    pub fn try_recompile_shader(
        &mut self,
        device: &Device,
    ) -> anyhow::Result<ShaderCompilationSuccess> {
        self.pipeline
            .try_recompile_shader(device, self.color_format)
            
    }

    pub fn render(&self, encoder: &mut CommandEncoder) {
        for render_param in &self.render_params {
            self.pipeline.render(
                encoder,
                &render_param.cube_target_texture_view,
                &self.mesh,
                &render_param.cube_face_viewproj_bind_group,
                &self.hdr_map_bind_group,
            );
        }
    }
}
