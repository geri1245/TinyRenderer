use core::f32::consts;

use glam::{Quat, Vec3};
use wgpu::{
    util::{align_to, DeviceExt},
    Buffer, BufferDescriptor, CommandEncoder, TextureViewDimension,
};

use crate::{
    bind_group_layout_descriptors,
    instance::{Instance, InstanceRaw},
    lights::{DirectionalLight, LightRaw, PointLight},
    model::Model,
    pipelines,
    texture::Texture,
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
    pub light_uniform_buffer: wgpu::Buffer,
    light_projection_only_uniform_buffer: wgpu::Buffer,
    uniform_buffer_alignment: u64,
    pub light_bind_group: wgpu::BindGroup,
    pub light_bind_group_viewproj_only: wgpu::BindGroup,
    // Used for drawing the debug visualizations of the lights
    pub light_instance_buffer: wgpu::Buffer,
    pub shadow_rp: pipelines::ShadowRP,
}

impl LightController {
    pub fn new(render_device: &wgpu::Device) -> LightController {
        // Make the `uniform_alignment` >= `light_uniform_size` and aligned to `min_uniform_buffer_offset_alignment`, as that is a requirement if we want to use dynamic offsets
        let matrix_size4x4 = core::mem::size_of::<[[f32; 4]; 4]>() as u64;
        let uniform_alignment = {
            let alignment =
                render_device.limits().min_uniform_buffer_offset_alignment as wgpu::BufferAddress;
            align_to(matrix_size4x4, alignment)
        };

        let light_projection_only_uniform_buffer = render_device.create_buffer(&BufferDescriptor {
            label: Some("Light uniform buffer"),
            size: uniform_alignment * NUM_OF_LIGHTS as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let light_uniform_size = core::mem::size_of::<LightRaw>() as wgpu::BufferAddress;
        let light_uniform_buffer = render_device.create_buffer(&BufferDescriptor {
            label: Some("Light uniform buffer"),
            size: light_uniform_size * NUM_OF_LIGHTS as u64,
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
                        buffer: &light_projection_only_uniform_buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(uniform_alignment),
                    }),
                }],
                label: Some("Light projection matrix only bind group"),
            });

        let shadow_texture =
            Texture::create_depth_texture(render_device, SHADOW_SIZE, "Shadow texture");

        let shadow_view = shadow_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor {
                array_layer_count: Some(NUM_OF_LIGHTS),
                dimension: Some(TextureViewDimension::D2Array),
                ..Default::default()
            });

        let mut shadow_target_views = (0..NUM_OF_LIGHTS)
            .map(|index| {
                shadow_texture
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor {
                        label: Some("shadow depth texture"),
                        format: Some(Texture::DEPTH_FORMAT),
                        dimension: Some(wgpu::TextureViewDimension::D2),
                        aspect: wgpu::TextureAspect::All,
                        base_mip_level: 0,
                        mip_level_count: None,
                        base_array_layer: index,
                        array_layer_count: Some(1),
                    })
            })
            .collect::<Vec<_>>();

        let shadow_rp =
            crate::pipelines::ShadowRP::new(&render_device, &shadow_texture, shadow_view);

        let directional_light: DirectionalLight = DirectionalLight::new(
            shadow_target_views.pop().unwrap(),
            Vec3::new(1.0, -1.0, 0.0).normalize(),
            Vec3::new(1.0, 1.0, 1.0),
        );

        let point_light = PointLight::new(
            shadow_target_views.pop().unwrap(),
            Vec3::new(20.0, 30.0, 0.0),
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::ZERO,
        );

        let light_instance_buffer =
            render_device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Light Instance buffer"),
                contents: bytemuck::cast_slice(&Self::get_raw_instances(&point_light)),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });

        Self {
            point_light,
            directional_light,
            light_uniform_buffer,
            light_bind_group,
            light_projection_only_uniform_buffer,
            light_bind_group_viewproj_only,
            light_instance_buffer,
            shadow_rp,
            uniform_buffer_alignment: uniform_alignment,
        }
    }

    pub fn update(&mut self, delta_time: std::time::Duration, render_queue: &wgpu::Queue) {
        let old_light_position = self.point_light.position;
        self.point_light.position = (Quat::from_axis_angle(
            (0.0, 1.0, 0.0).into(),
            consts::FRAC_PI_3 * delta_time.as_secs_f32(),
        ) * old_light_position)
            .into();

        render_queue.write_buffer(
            &self.light_uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.point_light.to_raw(), self.directional_light.to_raw()]),
        );

        render_queue.write_buffer(
            &self.light_projection_only_uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.point_light.to_raw()]),
        );

        render_queue.write_buffer(
            &self.light_projection_only_uniform_buffer,
            self.uniform_buffer_alignment,
            bytemuck::cast_slice(&[self.directional_light.to_raw()]),
        );

        render_queue.write_buffer(
            &self.light_instance_buffer,
            0,
            bytemuck::cast_slice(&Self::get_raw_instances(&self.point_light)),
        );
    }

    pub fn render_shadows(
        &self,
        encoder: &mut CommandEncoder,
        model: &Model,
        instance_count: usize,
        instance_buffer: &Buffer,
    ) {
        self.shadow_rp.render(
            encoder,
            model,
            &self.light_bind_group_viewproj_only,
            instance_count,
            instance_buffer,
            &self.point_light.depth_texture,
            0,
        );
        self.shadow_rp.render(
            encoder,
            model,
            &self.light_bind_group_viewproj_only,
            instance_count,
            instance_buffer,
            &self.directional_light.depth_texture,
            self.uniform_buffer_alignment as u32,
        );
    }

    fn get_raw_instances(light: &PointLight) -> Vec<InstanceRaw> {
        let light_instances = vec![Instance {
            position: light.position.into(),
            scale: Vec3::ONE,
            rotation: Quat::IDENTITY,
        }];
        light_instances
            .iter()
            .map(|instance| instance.to_raw())
            .collect::<Vec<_>>()
    }
}
