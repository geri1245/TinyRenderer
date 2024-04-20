use async_std::task::block_on;
use glam::Vec3;
use wgpu::{
    util::align_to, BindGroup, BufferDescriptor, CommandEncoder, Device, Extent3d,
    TextureViewDimension,
};

use crate::{
    bind_group_layout_descriptors,
    buffer::{create_bind_group_from_buffer_entire_binding, BufferBindGroupCreationOptions},
    instance::SceneComponentRaw,
    lights::{
        DirectionalLight, DirectionalLightRenderData, Light, LightRaw, LightRawSmall, PointLight,
        PointLightRenderData,
    },
    model::RenderableObject,
    pipelines::ShadowRP,
    texture::SampledTexture,
};

const SHADOW_SIZE: wgpu::Extent3d = wgpu::Extent3d {
    width: 1024,
    height: 1024,
    depth_or_array_layers: crate::renderer::MAX_LIGHTS as u32,
};

struct ShadowAssets {
    point_lights: Vec<PointLightRenderData>,
    directional_lights: Vec<DirectionalLightRenderData>,

    light_uniform_buffer: wgpu::Buffer,
    // This is used for creating the shadow maps. Here we are using dynamic offsets into the buffer,
    // so the data needs to be aligned properly. Thus we have 2 separate buffers
    light_viewproj_only_uniform_buffer: wgpu::Buffer,
    uniform_buffer_alignment: u64,
    light_bind_group: wgpu::BindGroup,
    light_bind_group_viewproj_only: wgpu::BindGroup,
    /// Contains the depth maps for the current lights
    shadow_bind_group: wgpu::BindGroup,
}

pub struct LightController {
    shadow_rp: ShadowRP,
    shadow_assets: ShadowAssets,
}

impl LightController {
    pub async fn new(device: &wgpu::Device) -> LightController {
        let lights = vec![
            Light::Point(PointLight::new(
                Vec3::new(10.0, 20.0, 0.0),
                Vec3::new(2.0, 5.0, 4.0),
            )),
            Light::Directional(DirectionalLight {
                direction: Vec3::new(1.0, -1.0, 1.0).normalize(),
                color: Vec3::new(1.0, 1.0, 1.0),
            }),
        ];

        // Make the `uniform_alignment` >= sizeof`LightRawSmall` and aligned to `min_uniform_buffer_offset_alignment`, as that is a requirement if we want to use dynamic offsets
        let matrix_size4x4 = core::mem::size_of::<LightRawSmall>() as u64;
        let uniform_alignment = {
            let alignment =
                device.limits().min_uniform_buffer_offset_alignment as wgpu::BufferAddress;
            align_to(matrix_size4x4, alignment)
        };

        let shadow_rp = crate::pipelines::ShadowRP::new(&device).await.unwrap();

        let shadow_assets = Self::create_shadow_assets(device, uniform_alignment, &lights);

        Self {
            shadow_rp,
            shadow_assets,
        }
    }

    pub fn get_shadow_bind_group(&self) -> &BindGroup {
        &self.shadow_assets.shadow_bind_group
    }

    pub fn get_light_bind_group(&self) -> &BindGroup {
        &self.shadow_assets.light_bind_group
    }

    fn create_shadow_assets(
        device: &wgpu::Device,
        uniform_alignment: u64,
        lights: &Vec<Light>,
    ) -> ShadowAssets {
        let (light_uniform_buffer, light_bind_group) =
            create_bind_group_from_buffer_entire_binding::<LightRaw>(
                device,
                &BufferBindGroupCreationOptions {
                    bind_group_layout_descriptor: &bind_group_layout_descriptors::LIGHT,
                    num_of_items: lights.len() as u64,
                    usages: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    label: "Light".into(),
                },
            );

        let (point_lights, point_shadow_texture) =
            Self::create_point_light_shadow_assets(device, lights);

        let point_shadow_view =
            point_shadow_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor {
                    array_layer_count: Some(6 * point_lights.len() as u32),
                    dimension: Some(TextureViewDimension::CubeArray),
                    aspect: wgpu::TextureAspect::DepthOnly,
                    ..Default::default()
                });

        let (directional_lights, directional_shadow_texture) =
            Self::create_directional_light_shadow_assets(device, lights);

        let directional_shadow_view =
            directional_shadow_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor {
                    array_layer_count: Some(directional_lights.len() as u32),
                    dimension: Some(TextureViewDimension::D2Array),
                    ..Default::default()
                });

        let shadow_bind_group = Self::create_bind_group(
            device,
            &directional_shadow_texture,
            &point_shadow_texture,
            directional_shadow_view,
            point_shadow_view,
        );

        let light_viewproj_only_uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Light uniform buffer"),
            size: uniform_alignment * (6 * point_lights.len() + directional_lights.len()) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let light_bind_group_viewproj_only = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &device.create_bind_group_layout(
                &bind_group_layout_descriptors::LIGHT_WITH_DYNAMIC_OFFSET,
            ),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &light_viewproj_only_uniform_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(uniform_alignment),
                }),
            }],
            label: Some("Light projection matrix only bind group"),
        });

        ShadowAssets {
            light_uniform_buffer,
            light_bind_group,
            light_viewproj_only_uniform_buffer,
            light_bind_group_viewproj_only,
            uniform_buffer_alignment: uniform_alignment,
            shadow_bind_group,
            point_lights,
            directional_lights,
        }
    }

    fn create_bind_group(
        device: &wgpu::Device,
        directional_shadow_texture: &SampledTexture,
        point_shadow_texture: &SampledTexture,
        directional_shadow_texture_view: wgpu::TextureView,
        point_shadow_texture_view: wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &device
                .create_bind_group_layout(&bind_group_layout_descriptors::SHADOW_DEPTH_TEXTURE),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&directional_shadow_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&directional_shadow_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&point_shadow_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&point_shadow_texture.sampler),
                },
            ],
            label: None,
        })
    }

    fn create_directional_light_shadow_assets(
        device: &Device,
        lights: &Vec<Light>,
    ) -> (Vec<DirectionalLightRenderData>, SampledTexture) {
        let directional_light_count = lights
            .iter()
            .filter(|light| {
                if let Light::Directional(_) = light {
                    true
                } else {
                    false
                }
            })
            .count();
        let directional_shadow_texture = SampledTexture::create_depth_texture(
            device,
            Extent3d {
                depth_or_array_layers: directional_light_count as u32,
                ..SHADOW_SIZE
            },
            "Directional shadow texture",
        );

        let mut directional_light_render_datas = Vec::new();

        for light in lights.iter() {
            if let Light::Directional(directional_light) = light {
                let directional_shadow_target_view = directional_shadow_texture
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor {
                        label: Some("shadow depth texture"),
                        format: Some(SampledTexture::DEPTH_FORMAT),
                        dimension: Some(wgpu::TextureViewDimension::D2),
                        aspect: wgpu::TextureAspect::All,
                        base_mip_level: 0,
                        mip_level_count: None,
                        base_array_layer: directional_light_render_datas.len() as u32,
                        array_layer_count: Some(1),
                    });

                let directional_light_render_data = DirectionalLightRenderData::new(
                    &directional_light,
                    directional_shadow_target_view,
                );

                directional_light_render_datas.push(directional_light_render_data);
            }
        }

        (directional_light_render_datas, directional_shadow_texture)
    }

    fn create_point_light_shadow_assets(
        device: &Device,
        lights: &Vec<Light>,
    ) -> (Vec<PointLightRenderData>, SampledTexture) {
        let point_light_count = lights
            .iter()
            .filter(|light| {
                if let Light::Point(_) = light {
                    true
                } else {
                    false
                }
            })
            .count();
        let point_light_texture = SampledTexture::create_depth_texture(
            device,
            Extent3d {
                depth_or_array_layers: 6 * point_light_count as u32,
                ..SHADOW_SIZE
            },
            "Point shadow texture",
        );

        let mut point_light_render_datas = Vec::new();

        for light in lights.iter() {
            if let Light::Point(point_light) = light {
                let point_light_shadow_target_view = (0..6)
                    .map(|face_index| {
                        point_light_texture
                            .texture
                            .create_view(&wgpu::TextureViewDescriptor {
                                label: Some("shadow cubemap texture view single face"),
                                format: Some(SampledTexture::DEPTH_FORMAT),
                                dimension: Some(wgpu::TextureViewDimension::D2),
                                aspect: wgpu::TextureAspect::All,
                                base_mip_level: 0,
                                mip_level_count: None,
                                base_array_layer: point_light_render_datas.len() as u32 * 6
                                    + face_index,
                                array_layer_count: Some(1),
                            })
                    })
                    .collect::<Vec<_>>();

                point_light_render_datas.push(PointLightRenderData::new(
                    point_light.clone(),
                    point_light_shadow_target_view,
                ));
            }
        }

        (point_light_render_datas, point_light_texture)
    }

    pub fn update(
        &mut self,
        _delta_time: std::time::Duration,
        render_queue: &wgpu::Queue,
        lights_if_any_was_dirty: Option<&Vec<Light>>,
    ) {
        render_queue.write_buffer(
            &self.shadow_assets.light_uniform_buffer,
            0,
            bytemuck::cast_slice(&[
                self.shadow_assets.point_lights[0].to_raw(),
                self.shadow_assets.directional_lights[0].to_raw(),
            ]),
        );

        for (point_light_index, point_light) in self.shadow_assets.point_lights.iter().enumerate() {
            let raw_viewprojs = point_light.get_viewprojs_raw();
            for (face_index, raw_data) in raw_viewprojs.iter().enumerate() {
                render_queue.write_buffer(
                    &self.shadow_assets.light_viewproj_only_uniform_buffer,
                    (point_light_index * 6 + face_index) as u64
                        * self.shadow_assets.uniform_buffer_alignment,
                    bytemuck::cast_slice(&[*raw_data]),
                );
            }
        }

        let base_offset_after_point_lights = 6
            * self.shadow_assets.point_lights.len()
            * self.shadow_assets.uniform_buffer_alignment as usize;

        render_queue.write_buffer(
            &self.shadow_assets.light_viewproj_only_uniform_buffer,
            base_offset_after_point_lights as u64,
            bytemuck::cast_slice(&[self.shadow_assets.directional_lights[0].to_raw_small()]),
        );
    }

    pub fn render_shadows(&self, encoder: &mut CommandEncoder, meshes: &Vec<RenderableObject>) {
        encoder.push_debug_group("Shadow rendering");

        {
            encoder.push_debug_group("Point shadows");

            for (light_index, light) in self.shadow_assets.point_lights.iter().enumerate() {
                for (face_index, depth_target) in light.depth_textures.iter().enumerate() {
                    self.shadow_rp.render(
                        encoder,
                        meshes,
                        &self.shadow_assets.light_bind_group_viewproj_only,
                        depth_target,
                        ((6 * light_index + face_index)
                            * self.shadow_assets.uniform_buffer_alignment as usize)
                            as u32,
                    );
                }
            }
            encoder.pop_debug_group();
        }
        {
            encoder.push_debug_group("Directional shadows");

            self.shadow_rp.render(
                encoder,
                meshes,
                &self.shadow_assets.light_bind_group_viewproj_only,
                &self.shadow_assets.directional_lights[0].depth_textures[0],
                6 * self.shadow_assets.uniform_buffer_alignment as u32,
            );

            encoder.pop_debug_group();
        }

        encoder.pop_debug_group();
    }

    pub fn try_recompile_shaders(&mut self, device: &Device) -> anyhow::Result<()> {
        block_on(self.shadow_rp.try_recompile_shader(device))?;

        Ok(())
    }

    pub fn get_raw_instances(light: &PointLight) -> Vec<SceneComponentRaw> {
        let light_instances = vec![light.transform];
        light_instances
            .iter()
            .map(|instance| instance.to_raw())
            .collect::<Vec<_>>()
    }
}
