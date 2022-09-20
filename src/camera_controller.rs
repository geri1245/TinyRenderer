use crate::camera::Camera;

use std::time;
use wgpu::util::DeviceExt;
use winit::event::DeviceEvent;

#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.0,
    0.0, 0.0, 0.5, 1.0,
);

/// Contains the rendering-related concepts of the camera
pub struct CameraController {
    camera: Camera,
    pub binding_buffer: wgpu::Buffer,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
    pub is_movement_enabled: bool,
}

impl CameraController {
    pub fn new(aspect_ratio: f32, render_device: &wgpu::Device) -> CameraController {
        let camera = Camera::new(aspect_ratio);

        let binding_buffer = render_device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[Self::get_raw(&camera)]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout =
            render_device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("camera_bind_group_layout"),
            });

        let bind_group = render_device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: binding_buffer.as_entire_binding(),
            }],
            label: Some("camera_bind_group"),
        });

        Self {
            camera,
            binding_buffer,
            bind_group_layout,
            bind_group,
            is_movement_enabled: false,
        }
    }

    pub fn resize(&mut self, aspect: f32) {
        self.camera.resize(aspect);
    }

    pub fn update(&mut self, delta_time: time::Duration, render_queue: &wgpu::Queue) {
        self.camera.update(delta_time);

        render_queue.write_buffer(
            &self.binding_buffer,
            0,
            bytemuck::cast_slice(&[self.to_raw()]),
        );
    }

    pub fn process_device_events(&mut self, event: DeviceEvent) {
        if self.is_movement_enabled {
            self.camera.process_device_events(event);
        }
    }

    pub fn to_raw(&self) -> CameraRaw {
        Self::get_raw(&self.camera)
    }

    fn get_raw(camera: &Camera) -> CameraRaw {
        let view = cgmath::Matrix4::look_at_rh(camera.position, camera.get_target(), camera.up);
        let proj = cgmath::perspective(camera.fovy, camera.aspect, camera.znear, camera.zfar);

        CameraRaw {
            view_proj: (OPENGL_TO_WGPU_MATRIX * proj * view).into(),
            camera_pos: camera.get_position().to_homogeneous().into(),
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraRaw {
    view_proj: [[f32; 4]; 4],
    camera_pos: [f32; 4],
}
