use std::{f32::consts, rc::Rc};

use glam::{Mat4, Vec3};
use wgpu::{CommandEncoder, Device, Texture, TextureFormat};

use crate::{
    bind_group_layout_descriptors,
    buffer::{
        create_bind_group_from_buffer_entire_binding_init, BufferInitBindGroupCreationOptions,
    },
    model::RenderableMesh,
    pipelines::EquirectangularToCubemapRP,
    texture::SampledTexture,
};

const CUBEMAP_RESOLUTION: u32 = 1024;

struct RenderParams {
    projection_bind_group: wgpu::BindGroup,
    cube_target_texture_view: wgpu::TextureView,
}

pub struct EquirectangularToCubemapRenderer {
    pipeline: EquirectangularToCubemapRP,
    mesh: Rc<RenderableMesh>,
    hdr_map_bind_group: wgpu::BindGroup,
    render_params: Vec<RenderParams>,
    pub cube_map_to_sample: Rc<wgpu::BindGroup>,
    color_format: wgpu::TextureFormat,
}

impl EquirectangularToCubemapRenderer {
    pub async fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: TextureFormat,
        basic_mesh: Rc<RenderableMesh>,
    ) -> anyhow::Result<Self> {
        let pipeline = EquirectangularToCubemapRP::new(device, color_format).await?;
        let hdr_texture_path = "assets/skybox/hdr/golf_course.hdr";
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
                &bind_group_layout_descriptors::TEXTURE_CUBE_FRAGMENT_WITH_SAMPLER,
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

        let render_params = Self::create_render_resources(device, &cube_texture);

        Ok(Self {
            pipeline,
            mesh: basic_mesh,
            hdr_map_bind_group,
            render_params,
            cube_map_to_sample: Rc::new(sampled_cubemap_bind_group),
            color_format,
        })
    }

    pub async fn try_recompile_shader(&mut self, device: &Device) -> anyhow::Result<()> {
        self.pipeline
            .try_recompile_shader(device, self.color_format)
            .await
    }

    pub fn render(&self, encoder: &mut CommandEncoder) {
        for render_param in &self.render_params {
            self.pipeline.render(
                encoder,
                &render_param.cube_target_texture_view,
                &self.mesh,
                &render_param.projection_bind_group,
                &self.hdr_map_bind_group,
            );
        }
    }

    fn create_render_resources(
        device: &Device,
        cube_target_texture: &Texture,
    ) -> Vec<RenderParams> {
        let proj = glam::Mat4::perspective_rh(consts::FRAC_PI_2, 1.0, 0.1, 2.0);

        const DIFF_AND_UP_VECTORS: [(Vec3, Vec3); 6] = [
            (Vec3::X, Vec3::Y),
            (Vec3::NEG_X, Vec3::Y),
            (Vec3::Y, Vec3::NEG_Z),
            (Vec3::NEG_Y, Vec3::Z),
            (Vec3::Z, Vec3::Y),
            (Vec3::NEG_Z, Vec3::Y),
        ];

        DIFF_AND_UP_VECTORS
            .iter()
            .enumerate()
            .map(|(index, &(diff, up))| {
                let view = Mat4::look_at_rh(Vec3::ZERO, diff, up);
                let matrix_data = (proj * view).to_cols_array();

                let (_buffer, bind_group) = create_bind_group_from_buffer_entire_binding_init(
                    device,
                    &BufferInitBindGroupCreationOptions {
                        bind_group_layout_descriptor:
                            &bind_group_layout_descriptors::BUFFER_VISIBLE_EVERYWHERE,
                        usages: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                        label: "Equirectangular projection viewprojs".into(),
                    },
                    bytemuck::cast_slice(&matrix_data),
                );

                let view = cube_target_texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("HDR cubemap target view"),
                    base_array_layer: index as u32,
                    array_layer_count: Some(1),
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_mip_level: 0,
                    mip_level_count: None,
                    ..Default::default()
                });

                RenderParams {
                    cube_target_texture_view: view,
                    projection_bind_group: bind_group,
                }
            })
            .collect()
    }
}
