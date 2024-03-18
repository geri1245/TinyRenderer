use async_std::task::block_on;
use glam::Vec3;
use wgpu::{
    util::align_to, BufferDescriptor, CommandEncoder, Device, Extent3d, TextureViewDimension,
};

use crate::{
    bind_group_layout_descriptors,
    instance::SceneComponentRaw,
    lights::{DirectionalLight, LightRaw, LightRawSmall, PointLight},
    model::InstancedTexturedRenderableMesh,
    pipelines::{self, PipelineRecreationResult},
    texture::SampledTexture,
    world::World,
    world_renderer::MeshType,
};

// TODO: Wherever this is used, dinamically calculate the number of lights
const NUM_OF_LIGHTS: u32 = 2u32;

const SHADOW_SIZE: wgpu::Extent3d = wgpu::Extent3d {
    width: 2048,
    height: 2048,
    depth_or_array_layers: crate::renderer::MAX_LIGHTS as u32,
};

pub struct LightController {
    pub point_light: PointLight,
    pub directional_light: DirectionalLight,
    light_uniform_buffer: wgpu::Buffer,
    // This is used for creating the shadow maps. Here we are using dynamic offsets into the buffer,
    // so the data needs to be aligned properly. Thus we have 2 separate buffers
    light_viewproj_only_uniform_buffer: wgpu::Buffer,
    uniform_buffer_alignment: u64,
    pub light_bind_group: wgpu::BindGroup,
    pub light_bind_group_viewproj_only: wgpu::BindGroup,
    pub shadow_rp: pipelines::ShadowRP,
    pub debug_light_meshes: Vec<InstancedTexturedRenderableMesh>,
    /// Contains the depth maps for the current lights
    pub shadow_bind_group: wgpu::BindGroup,
}

impl LightController {
    pub async fn new(device: &wgpu::Device, world: &mut World) -> LightController {
        // Make the `uniform_alignment` >= `light_uniform_size` and aligned to `min_uniform_buffer_offset_alignment`, as that is a requirement if we want to use dynamic offsets
        let matrix_size4x4 = core::mem::size_of::<LightRawSmall>() as u64;
        let uniform_alignment = {
            let alignment =
                device.limits().min_uniform_buffer_offset_alignment as wgpu::BufferAddress;
            align_to(matrix_size4x4, alignment)
        };

        let light_viewproj_only_uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Light uniform buffer"),
            size: uniform_alignment * 7 as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let light_raw_size = core::mem::size_of::<LightRaw>() as wgpu::BufferAddress;
        let light_uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Light uniform buffer"),
            size: light_raw_size * NUM_OF_LIGHTS as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let light_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &device.create_bind_group_layout(&bind_group_layout_descriptors::LIGHT),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: light_uniform_buffer.as_entire_binding(),
            }],
            label: Some("Light bind group"),
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

        let shadow_rp = crate::pipelines::ShadowRP::new(&device).await.unwrap();

        let (point_light, point_shadow_texture) = Self::create_point_light_shadow_assets(device);

        let point_shadow_view =
            point_shadow_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor {
                    array_layer_count: Some(6),
                    dimension: Some(TextureViewDimension::Cube),
                    aspect: wgpu::TextureAspect::DepthOnly,
                    ..Default::default()
                });

        let (directional_light, directional_shadow_texture) =
            Self::create_directional_light_shadow_assets(device);

        let directional_shadow_view =
            directional_shadow_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor {
                    array_layer_count: Some(NUM_OF_LIGHTS),
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

        world.add_debug_object(crate::world::DebugObjectType::Cube, &point_light.transform);

        Self {
            point_light,
            directional_light,
            light_uniform_buffer,
            light_bind_group,
            light_viewproj_only_uniform_buffer,
            light_bind_group_viewproj_only,
            shadow_rp,
            uniform_buffer_alignment: uniform_alignment,
            debug_light_meshes: vec![],
            shadow_bind_group,
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
    ) -> (DirectionalLight, SampledTexture) {
        let directional_shadow_texture = SampledTexture::create_depth_texture(
            device,
            Extent3d { ..SHADOW_SIZE },
            "Directional shadow texture",
        );

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
                    base_array_layer: 0,
                    array_layer_count: Some(1),
                });

        let directional_light: DirectionalLight = DirectionalLight::new(
            directional_shadow_target_view,
            Vec3::new(1.0, -1.0, 0.0).normalize(),
            Vec3::new(1.0, 1.0, 1.0),
        );

        (directional_light, directional_shadow_texture)
    }

    fn create_point_light_shadow_assets(device: &Device) -> (PointLight, SampledTexture) {
        let point_light_texture = SampledTexture::create_depth_texture(
            device,
            Extent3d {
                depth_or_array_layers: 6,
                ..SHADOW_SIZE
            },
            "Point shadow texture",
        );

        let point_light_shadow_target_view = (0..6)
            .map(|index| {
                point_light_texture
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor {
                        label: Some("shadow cubemap texture view single face"),
                        format: Some(SampledTexture::DEPTH_FORMAT),
                        dimension: Some(wgpu::TextureViewDimension::D2),
                        aspect: wgpu::TextureAspect::All,
                        base_mip_level: 0,
                        mip_level_count: None,
                        base_array_layer: index,
                        array_layer_count: Some(1),
                    })
            })
            .collect::<Vec<_>>();

        let point_light = PointLight::new(
            point_light_shadow_target_view,
            Vec3::new(30.0, 40.0, 0.0),
            Vec3::new(5.0, 10.0, 10.0),
            Vec3::ZERO,
        );

        (point_light, point_light_texture)
    }

    pub fn set_light_position(&mut self, new_position: Vec3) {
        self.point_light.transform.position = new_position;
    }

    pub fn update(&mut self, _delta_time: std::time::Duration, render_queue: &wgpu::Queue) {
        render_queue.write_buffer(
            &self.light_uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.point_light.to_raw(), self.directional_light.to_raw()]),
        );

        let raw_viewprojs = self.point_light.get_viewprojs_raw();
        for i in 0..6 {
            render_queue.write_buffer(
                &self.light_viewproj_only_uniform_buffer,
                i * self.uniform_buffer_alignment,
                bytemuck::cast_slice(&[raw_viewprojs[i as usize]]),
            );
        }

        render_queue.write_buffer(
            &self.light_viewproj_only_uniform_buffer,
            6 * self.uniform_buffer_alignment,
            bytemuck::cast_slice(&[self.directional_light.to_raw()]),
        );
    }

    pub fn render_shadows(&self, encoder: &mut CommandEncoder, meshes: &Vec<MeshType>) {
        for i in 0..6 {
            self.shadow_rp.render(
                encoder,
                meshes,
                &self.light_bind_group_viewproj_only,
                &self.point_light.depth_texture[i],
                (i as u64 * self.uniform_buffer_alignment) as u32,
            );
        }
        self.shadow_rp.render(
            encoder,
            meshes,
            &self.light_bind_group_viewproj_only,
            &self.directional_light.depth_texture,
            6 * self.uniform_buffer_alignment as u32,
        );
    }

    pub fn try_recompile_shaders(&mut self, device: &Device) -> anyhow::Result<()> {
        let result = block_on(self.shadow_rp.try_recompile_shader(device));
        match result {
            PipelineRecreationResult::AlreadyUpToDate => Ok(()),
            PipelineRecreationResult::Success(new_pipeline) => {
                self.shadow_rp = new_pipeline;
                Ok(())
            }
            PipelineRecreationResult::Failed(error) => Err(error),
        }
    }

    pub fn get_raw_instances(light: &PointLight) -> Vec<SceneComponentRaw> {
        let light_instances = vec![light.transform];
        light_instances
            .iter()
            .map(|instance| instance.to_raw())
            .collect::<Vec<_>>()
    }
}
