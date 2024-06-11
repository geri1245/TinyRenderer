use async_std::task::block_on;
use glam::Vec3;
use wgpu::{
    util::align_to, BindGroup, CommandEncoder, Device, Extent3d, TextureView, TextureViewDimension,
};

use crate::{
    bind_group_layout_descriptors,
    buffer::{
        create_bind_group_from_buffer_entire_binding,
        create_bind_group_from_buffer_entire_binding_fixed_size,
        create_bind_group_from_buffer_entire_binding_init, BufferBindGroupCreationOptions,
        BufferInitBindGroupCreationOptions,
    },
    instance::SceneComponentRaw,
    lights::{
        DirectionalLight, DirectionalLightRenderData, Light, LightRaw, LightRawSmall, PointLight,
        PointLightRenderData,
    },
    model::RenderableObject,
    pipelines::ShadowRP,
    texture::SampledTexture,
    world::World,
};

const SHADOW_SIZE: wgpu::Extent3d = wgpu::Extent3d {
    width: 1024,
    height: 1024,
    depth_or_array_layers: crate::renderer::MAX_LIGHTS as u32,
};

#[repr(C)]
#[derive(Default, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct LightCount {
    point: u32,
    directional: u32,
}

struct PointLightShadowTextureBudget {
    depth_texture: SampledTexture,
    /// Depth render targets for the lights. The inner vector will have 6 components always (could use an array instead of a vec to be more readable)
    depth_render_target_views: Vec<Vec<wgpu::TextureView>>,
    /// Cube array view to read the light shadow maps
    depth_view: wgpu::TextureView,
}

struct DirectionalLightShadowAssets {
    render_data: Vec<DirectionalLightRenderData>,
    shadow_texture: SampledTexture,
    shadow_texture_view: TextureView,
}

struct ShadowAssets {
    point_light_texture_budget: PointLightShadowTextureBudget,
    point_lights: Vec<PointLightRenderData>,
    directional_lights: DirectionalLightShadowAssets,

    light_uniform_buffer: wgpu::Buffer,
    light_bind_group: wgpu::BindGroup,

    light_parameters_uniform_buffer: wgpu::Buffer,
    light_parameters_bind_group: wgpu::BindGroup,

    // This is used for creating the shadow maps. Here we are using dynamic offsets into the buffer,
    // so the data needs to be aligned properly. Thus we have 2 separate buffers
    light_viewproj_only_uniform_buffer: wgpu::Buffer,
    light_bind_group_viewproj_only: wgpu::BindGroup,

    uniform_buffer_alignment: u64,
    /// Contains the depth maps for the current lights
    shadow_bind_group: wgpu::BindGroup,
}

pub struct LightController {
    shadow_rp: ShadowRP,
    shadow_assets: ShadowAssets,
}

impl LightController {
    pub async fn new(device: &wgpu::Device) -> LightController {
        // Make the `uniform_alignment` >= sizeof`LightRawSmall` and aligned to `min_uniform_buffer_offset_alignment`, as that is a requirement if we want to use dynamic offsets
        let matrix_size4x4 = core::mem::size_of::<LightRawSmall>() as u64;
        let uniform_alignment = {
            let alignment =
                device.limits().min_uniform_buffer_offset_alignment as wgpu::BufferAddress;
            align_to(matrix_size4x4, alignment)
        };

        let shadow_rp = crate::pipelines::ShadowRP::new(&device).await.unwrap();

        // Here we just pass in some random lights, so the buffers and bind groups can be created
        // Providing 0 for the buffer size is a wgpu error, and this would happen in case of an empty vector,
        // so the simplest thing is to just fake in some lights here
        let shadow_assets = Self::create_initial_shadow_assets(device, uniform_alignment);

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

    pub fn get_light_parameters_bind_group(&self) -> &BindGroup {
        &self.shadow_assets.light_parameters_bind_group
    }

    fn create_initial_shadow_assets(device: &wgpu::Device, uniform_alignment: u64) -> ShadowAssets {
        let lights = vec![
            Light::Point(PointLight::new(Vec3::ZERO, Vec3::ZERO)),
            Light::Directional(DirectionalLight {
                color: Vec3::ZERO,
                direction: Vec3::ZERO,
            }),
        ];

        let (point_lights, directional_lights) = Self::categorize_lights(&lights);

        let mut shadow_assets = Self::create_shadow_assets(
            device,
            uniform_alignment,
            &point_lights,
            &directional_lights,
        );

        // Clear the faked assets, so we are not rendering anything with them
        shadow_assets.directional_lights.render_data.clear();

        shadow_assets
    }

    fn update_shadow_assets_if_needed(&mut self, device: &wgpu::Device, lights: &Vec<Light>) {
        // let light_counts = Self::get_light_params(lights);

        // if light_counts.point > self.shadow_assets.point_lights.render_data.capacity() as u32
        //     || light_counts.directional
        //         > self.shadow_assets.directional_lights.render_data.capacity() as u32
        {
            let (point_lights, directional_lights) = Self::categorize_lights(&lights);

            self.shadow_assets = Self::create_shadow_assets(
                device,
                self.shadow_assets.uniform_buffer_alignment,
                &point_lights,
                &directional_lights,
            );
        }
    }

    fn categorize_lights(lights: &Vec<Light>) -> (Vec<PointLight>, Vec<DirectionalLight>) {
        let mut point_lights = Vec::new();
        let mut directional_lights = Vec::new();

        for light in lights {
            match light {
                Light::Point(point) => point_lights.push(point.clone()),
                Light::Directional(directional) => directional_lights.push(directional.clone()),
            }
        }

        (point_lights, directional_lights)
    }

    fn create_shadow_assets(
        device: &wgpu::Device,
        uniform_alignment: u64,
        point_lights: &Vec<PointLight>,
        directional_lights: &Vec<DirectionalLight>,
    ) -> ShadowAssets {
        let (light_uniform_buffer, light_bind_group) =
            create_bind_group_from_buffer_entire_binding::<LightRaw>(
                device,
                &BufferBindGroupCreationOptions {
                    bind_group_layout_descriptor: &bind_group_layout_descriptors::LIGHT,
                    num_of_items: (point_lights.len() + directional_lights.len()) as u64,
                    usages: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    label: "Light".into(),
                    binding_size: None,
                },
            );

        let light_params = LightCount {
            directional: directional_lights.len() as u32,
            point: point_lights.len() as u32,
        };

        let (light_parameters_uniform_buffer, light_parameters_bind_group) =
            create_bind_group_from_buffer_entire_binding_init(
                device,
                &BufferInitBindGroupCreationOptions {
                    bind_group_layout_descriptor:
                        &bind_group_layout_descriptors::BUFFER_VISIBLE_EVERYWHERE,
                    usages: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    label: "Light parameters".into(),
                },
                bytemuck::cast_slice(&[light_params]),
            );

        let point_light_shadow_budget =
            Self::create_point_light_depth_targets(device, point_lights.len() as u32);

        let point_light_render_datas = Self::create_point_light_render_datas(&point_lights);

        let directional_light_shadow_data =
            Self::create_directional_light_shadow_assets(device, directional_lights);

        let shadow_bind_group = Self::create_bind_group(
            device,
            &directional_light_shadow_data.shadow_texture,
            &point_light_shadow_budget.depth_texture,
            &directional_light_shadow_data.shadow_texture_view,
            &point_light_shadow_budget.depth_view,
        );

        let (light_viewproj_only_uniform_buffer, light_bind_group_viewproj_only) =
            create_bind_group_from_buffer_entire_binding_fixed_size(
                device,
                &BufferBindGroupCreationOptions {
                    bind_group_layout_descriptor:
                        &bind_group_layout_descriptors::BUFFER_WITH_DYNAMIC_OFFSET,
                    num_of_items: 6 * point_lights.len() as u64 + directional_lights.len() as u64,
                    usages: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    label: "Light projection matrix only".into(),
                    binding_size: Some(uniform_alignment),
                },
                uniform_alignment,
            );

        ShadowAssets {
            light_uniform_buffer,
            light_bind_group,
            light_parameters_bind_group,
            light_parameters_uniform_buffer,
            light_viewproj_only_uniform_buffer,
            light_bind_group_viewproj_only,
            uniform_buffer_alignment: uniform_alignment,
            shadow_bind_group,
            point_light_texture_budget: point_light_shadow_budget,
            directional_lights: directional_light_shadow_data,
            point_lights: point_light_render_datas,
        }
    }

    fn create_bind_group(
        device: &wgpu::Device,
        directional_shadow_texture: &SampledTexture,
        point_shadow_texture: &SampledTexture,
        directional_shadow_texture_view: &wgpu::TextureView,
        point_shadow_texture_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &device
                .create_bind_group_layout(&bind_group_layout_descriptors::SHADOW_DEPTH_TEXTURE),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(directional_shadow_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&directional_shadow_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(point_shadow_texture_view),
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
        lights: &Vec<DirectionalLight>,
    ) -> DirectionalLightShadowAssets {
        let directional_shadow_texture = SampledTexture::create_depth_texture(
            device,
            Extent3d {
                depth_or_array_layers: lights.len() as u32,
                ..SHADOW_SIZE
            },
            "Directional shadow texture",
        );

        let mut directional_light_render_datas = Vec::new();

        for light in lights.iter() {
            let directional_shadow_target_view =
                directional_shadow_texture
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

            let directional_light_render_data =
                DirectionalLightRenderData::new(&light, directional_shadow_target_view);

            directional_light_render_datas.push(directional_light_render_data);
        }

        let directional_shadow_view =
            directional_shadow_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor {
                    array_layer_count: Some(lights.len() as u32),
                    dimension: Some(TextureViewDimension::D2Array),
                    ..Default::default()
                });

        DirectionalLightShadowAssets {
            render_data: directional_light_render_datas,
            shadow_texture: directional_shadow_texture,
            shadow_texture_view: directional_shadow_view,
        }
    }

    fn create_point_light_depth_targets(
        device: &Device,
        point_light_count: u32,
    ) -> PointLightShadowTextureBudget {
        let depth_texture = SampledTexture::create_depth_texture(
            device,
            Extent3d {
                depth_or_array_layers: 6 * point_light_count,
                ..SHADOW_SIZE
            },
            "Point shadow texture",
        );

        // Map through each light index and through each cube face for each light and create
        // the depth target views to render the shadow map into
        let depth_render_target_views = (0..point_light_count)
            .map(|light_index| {
                (0..6)
                    .map(|face_index| {
                        depth_texture
                            .texture
                            .create_view(&wgpu::TextureViewDescriptor {
                                label: Some("shadow cubemap texture view single face"),
                                format: Some(SampledTexture::DEPTH_FORMAT),
                                dimension: Some(wgpu::TextureViewDimension::D2),
                                aspect: wgpu::TextureAspect::All,
                                base_mip_level: 0,
                                mip_level_count: None,
                                base_array_layer: light_index * 6 + face_index,
                                array_layer_count: Some(1),
                            })
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<Vec<_>>>();

        // Create the cube array view for reading the shadow map of the point lights
        let depth_view = depth_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor {
                array_layer_count: Some(6 * point_light_count as u32),
                dimension: Some(TextureViewDimension::CubeArray),
                aspect: wgpu::TextureAspect::DepthOnly,
                ..Default::default()
            });

        PointLightShadowTextureBudget {
            depth_texture,
            depth_render_target_views,
            depth_view,
        }
    }

    fn create_point_light_render_datas(lights: &Vec<PointLight>) -> Vec<PointLightRenderData> {
        lights
            .iter()
            .enumerate()
            .map(|(index, light)| PointLightRenderData::new(light.clone(), index))
            .collect()
    }

    fn update_lights_gpu_data(&mut self, render_queue: &wgpu::Queue, lights: &Vec<Light>) {
        let (point_lights, directional_lights) = Self::categorize_lights(lights);
        self.shadow_assets.point_lights = Self::create_point_light_render_datas(&point_lights);
        let mut light_raws = self
            .shadow_assets
            .point_lights
            .iter()
            .map(|light| light.to_raw())
            .collect::<Vec<_>>();

        for directional_light in &self.shadow_assets.directional_lights.render_data {
            light_raws.push(directional_light.to_raw())
        }

        render_queue.write_buffer(
            &self.shadow_assets.light_uniform_buffer,
            0,
            bytemuck::cast_slice(&light_raws),
        );

        let light_params = LightCount {
            point: self.shadow_assets.point_lights.len() as u32,
            directional: self.shadow_assets.directional_lights.render_data.len() as u32,
        };

        render_queue.write_buffer(
            &self.shadow_assets.light_parameters_uniform_buffer,
            0,
            bytemuck::cast_slice(&[light_params]),
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

        for (directional_light_index, directional_light) in self
            .shadow_assets
            .directional_lights
            .render_data
            .iter()
            .enumerate()
        {
            render_queue.write_buffer(
                &self.shadow_assets.light_viewproj_only_uniform_buffer,
                base_offset_after_point_lights as u64
                    + self.shadow_assets.uniform_buffer_alignment * directional_light_index as u64,
                bytemuck::cast_slice(&[directional_light.to_raw_small()]),
            );
        }
    }

    pub fn update(
        &mut self,
        _delta_time: std::time::Duration,
        render_queue: &wgpu::Queue,
        device: &Device,
        world: &World,
    ) {
        match world.get_lights_dirty_state() {
            crate::world::DirtyState::NothingChanged => {}
            crate::world::DirtyState::ItemsChanged => {
                self.update_shadow_assets_if_needed(device, world.get_lights());
                self.update_lights_gpu_data(render_queue, world.get_lights());
            }
            crate::world::DirtyState::ItemPropertiesChanged => {
                self.update_shadow_assets_if_needed(device, world.get_lights());
                self.update_lights_gpu_data(render_queue, world.get_lights());
            }
        }
    }

    pub fn render_shadows(&self, encoder: &mut CommandEncoder, meshes: &Vec<RenderableObject>) {
        encoder.push_debug_group("Shadow rendering");

        {
            encoder.push_debug_group("Point shadows");

            for (light_index, light) in self.shadow_assets.point_lights.iter().enumerate() {
                for (face_index, depth_target) in self
                    .shadow_assets
                    .point_light_texture_budget
                    .depth_render_target_views[light.depth_texture_index]
                    .iter()
                    .enumerate()
                {
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

        let base_offset_after_point_lights = 6
            * self.shadow_assets.point_lights.len()
            * self.shadow_assets.uniform_buffer_alignment as usize;

        {
            encoder.push_debug_group("Directional shadows");

            for (light_index, light) in self
                .shadow_assets
                .directional_lights
                .render_data
                .iter()
                .enumerate()
            {
                self.shadow_rp.render(
                    encoder,
                    meshes,
                    &self.shadow_assets.light_bind_group_viewproj_only,
                    &light.depth_texture,
                    (base_offset_after_point_lights
                        + light_index * self.shadow_assets.uniform_buffer_alignment as usize)
                        as u32,
                );
            }

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
