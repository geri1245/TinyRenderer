use core::f32::consts;

use glam::{Quat, Vec3};
use wgpu::util::DeviceExt;

use crate::{
    bind_group_layout_descriptors,
    instance::{Instance, InstanceRaw},
    pipelines,
    point_light::PointLight,
};

const DEFAULT_LIGHT: PointLight = PointLight {
    position: [20.0, 30.0, 0.0],
    color: [1.0, 1.0, 1.0],
    target: [0.0, 0.0, 0.0],
};

pub struct LightController {
    pub light: PointLight,
    pub uniform_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    // Used for drawing the debug visualizations of the lights
    pub light_instance_buffer: wgpu::Buffer,
    pub shadow_rp: pipelines::ShadowRP,
}

impl LightController {
    pub fn new(render_device: &wgpu::Device) -> LightController {
        let buffer = render_device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light Uniform Buffer"),
            contents: bytemuck::cast_slice(&[DEFAULT_LIGHT.to_raw()]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let light_instance_buffer =
            render_device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Light Instance buffer"),
                contents: bytemuck::cast_slice(&Self::get_raw_instances(&DEFAULT_LIGHT)),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });

        let bind_group = render_device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &render_device.create_bind_group_layout(&bind_group_layout_descriptors::LIGHT),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
            label: Some("Light bind group"),
        });

        let shadow_rp = crate::pipelines::ShadowRP::new(&render_device);

        Self {
            light: DEFAULT_LIGHT.clone(),
            uniform_buffer: buffer,
            bind_group,
            light_instance_buffer,
            shadow_rp,
        }
    }

    pub fn update(&mut self, delta_time: std::time::Duration, render_queue: &wgpu::Queue) {
        let old_light_position = Vec3::from_array(self.light.position);
        self.light.position = (Quat::from_axis_angle(
            (0.0, 1.0, 0.0).into(),
            consts::FRAC_PI_3 * delta_time.as_secs_f32(),
        ) * old_light_position)
            .into();

        render_queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.light.to_raw()]),
        );

        render_queue.write_buffer(
            &self.light_instance_buffer,
            0,
            bytemuck::cast_slice(&Self::get_raw_instances(&self.light)),
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
