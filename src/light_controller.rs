use async_std::task::block_on;
use glam::{Quat, Vec3};
use wgpu::{
    util::{align_to, DeviceExt},
    BufferDescriptor, CommandEncoder, Device, Extent3d, TextureViewDimension,
};

use crate::{
    bind_group_layout_descriptors,
    instance::{Instance, InstanceRaw},
    lights::{DirectionalLight, LightRaw, LightRawSmall, PointLight},
    model::InstancedRenderableMesh,
    pipelines::{self, PipelineRecreationResult},
    texture::SampledTexture,
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
    // Used for drawing the debug visualizations of the lights
    pub light_instance_buffer: wgpu::Buffer,
    pub shadow_rp: pipelines::ShadowRP,
    pub debug_light_meshes: Vec<InstancedRenderableMesh>,
}

impl LightController {
    pub async fn new(render_device: &wgpu::Device) -> LightController {
        // Make the `uniform_alignment` >= `light_uniform_size` and aligned to `min_uniform_buffer_offset_alignment`, as that is a requirement if we want to use dynamic offsets
        let matrix_size4x4 = core::mem::size_of::<LightRawSmall>() as u64;
        let uniform_alignment = {
            let alignment =
                render_device.limits().min_uniform_buffer_offset_alignment as wgpu::BufferAddress;
            align_to(matrix_size4x4, alignment)
        };

        let light_viewproj_only_uniform_buffer = render_device.create_buffer(&BufferDescriptor {
            label: Some("Light uniform buffer"),
            size: uniform_alignment * 7 as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let light_raw_size = core::mem::size_of::<LightRaw>() as wgpu::BufferAddress;
        let light_uniform_buffer = render_device.create_buffer(&BufferDescriptor {
            label: Some("Light uniform buffer"),
            size: light_raw_size * NUM_OF_LIGHTS as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let light_bind_group = render_device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &render_device.create_bind_group_layout(&bind_group_layout_descriptors::LIGHT),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: light_uniform_buffer.as_entire_binding(),
            }],
            label: Some("Light bind group"),
        });

        let light_bind_group_viewproj_only =
            render_device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &render_device.create_bind_group_layout(
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

        let directional_shadow_texture = SampledTexture::create_depth_texture(
            render_device,
            Extent3d { ..SHADOW_SIZE },
            "Directional shadow texture",
        );
        let point_light_texture = SampledTexture::create_depth_texture(
            render_device,
            Extent3d {
                depth_or_array_layers: 6,
                ..SHADOW_SIZE
            },
            "Point shadow texture",
        );

        let directional_shadow_view =
            directional_shadow_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor {
                    array_layer_count: Some(NUM_OF_LIGHTS),
                    dimension: Some(TextureViewDimension::D2Array),
                    ..Default::default()
                });

        let point_shadow_view =
            point_light_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor {
                    array_layer_count: Some(6),
                    dimension: Some(TextureViewDimension::Cube),
                    aspect: wgpu::TextureAspect::DepthOnly,
                    ..Default::default()
                });

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

        let shadow_rp = crate::pipelines::ShadowRP::new(
            &render_device,
            directional_shadow_texture,
            point_light_texture,
            directional_shadow_view,
            point_shadow_view,
        )
        .await
        .unwrap();

        let directional_light: DirectionalLight = DirectionalLight::new(
            directional_shadow_target_view,
            Vec3::new(1.0, -1.0, 0.0).normalize(),
            Vec3::new(1.0, 1.0, 1.0),
        );

        let point_light = PointLight::new(
            point_light_shadow_target_view,
            Vec3::new(10.0, 20.0, 0.0),
            Vec3::new(25.0, 20.0, 20.0),
            Vec3::ZERO,
        );

        let light_instance_buffer =
            render_device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Light Instance buffer"),
                contents: bytemuck::cast_slice(&Self::get_raw_instances(&point_light)),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });

        // let debug_light_meshes = vec![InstancedRenderableMesh::new(render_device, , vec![])];

        Self {
            point_light,
            directional_light,
            light_uniform_buffer,
            light_bind_group,
            light_viewproj_only_uniform_buffer,
            light_bind_group_viewproj_only,
            light_instance_buffer,
            shadow_rp,
            uniform_buffer_alignment: uniform_alignment,
            debug_light_meshes: vec![],
        }
    }

    pub fn set_light_position(&mut self, new_position: Vec3) {
        self.point_light.position = new_position;
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

        render_queue.write_buffer(
            &self.light_instance_buffer,
            0,
            bytemuck::cast_slice(&Self::get_raw_instances(&self.point_light)),
        );
    }

    pub fn render_shadows(&self, encoder: &mut CommandEncoder, mesh: &InstancedRenderableMesh) {
        for i in 0..6 {
            self.shadow_rp.render(
                encoder,
                mesh,
                &self.light_bind_group_viewproj_only,
                &self.point_light.depth_texture[i],
                (i as u64 * self.uniform_buffer_alignment) as u32,
            );
        }
        self.shadow_rp.render(
            encoder,
            mesh,
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

    pub fn get_raw_instances(light: &PointLight) -> Vec<InstanceRaw> {
        let light_instances = vec![Instance {
            position: light.position.into(),
            scale: Vec3::splat(0.1),
            rotation: Quat::IDENTITY,
        }];
        light_instances
            .iter()
            .map(|instance| instance.to_raw())
            .collect::<Vec<_>>()
    }
}
