use glam::{Mat4, Vec3, Vec4, Vec4Swizzles};
use std::time;
use wgpu::Device;
use winit::{
    dpi::PhysicalPosition,
    event::{MouseButton, WindowEvent},
};

use math_helpers::reverse_z_matrix;

use crate::{
    bind_group_layout_descriptors,
    buffer::{create_bind_group_from_buffer_entire_binding_init, GpuBufferCreationOptions},
    camera::{Camera, CameraEvent},
};

/// Contains the rendering-related concepts of the camera
pub struct CameraController {
    pub camera: Camera,
    pub binding_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    is_movement_enabled: bool,
    cursor_position: Option<PhysicalPosition<f64>>,

    width: u32,
    height: u32,
}

impl CameraController {
    pub fn new(device: &Device, width: u32, height: u32) -> CameraController {
        let camera = Camera::new(width, height);

        Self::from_camera(device, &camera, width, height)
    }

    pub fn from_camera(device: &Device, camera: &Camera, width: u32, height: u32) -> Self {
        let (binding_buffer, bind_group) = create_bind_group_from_buffer_entire_binding_init(
            device,
            &GpuBufferCreationOptions {
                bind_group_layout_descriptor:
                    &bind_group_layout_descriptors::BUFFER_VISIBLE_EVERYWHERE,
                usages: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                label: "Camera".into(),
            },
            bytemuck::cast_slice(&[Self::get_raw(camera)]),
        );

        Self {
            camera: camera.clone(),
            binding_buffer,
            bind_group,
            is_movement_enabled: false,
            cursor_position: None,
            width,
            height,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.camera.resize(width, height);

        self.width = width;
        self.height = height;
    }

    pub fn update(&mut self, delta_time: time::Duration, render_queue: &wgpu::Queue) {
        self.camera.update(delta_time);

        render_queue.write_buffer(
            &self.binding_buffer,
            0,
            bytemuck::cast_slice(&[self.to_raw()]),
        );
    }

    pub fn set_is_movement_enabled(&mut self, value: bool) {
        self.is_movement_enabled = value;

        if !self.is_movement_enabled {
            self.camera.stop_movement();
            self.cursor_position = None;
        }
    }

    pub fn process_window_event(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::MouseInput { button, state, .. } => {
                if *button == MouseButton::Right {
                    self.set_is_movement_enabled(state.is_pressed());
                    return true;
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if self.is_movement_enabled {
                    if let Some(previous_position) = self.cursor_position {
                        self.camera.process_event(&CameraEvent::Motion((
                            position.x - previous_position.x,
                            position.y - previous_position.y,
                        )));
                    }

                    self.cursor_position = Some(*position);
                    return true;
                }
            }
            WindowEvent::KeyboardInput { event, .. } if self.is_movement_enabled => {
                self.camera.process_event(&CameraEvent::Key(event.clone()));
                return true;
            }
            _ => {}
        }

        false
    }

    pub fn to_raw(&self) -> CameraRaw {
        Self::get_raw(&self.camera)
    }

    fn get_raw(camera: &Camera) -> CameraRaw {
        let view = Mat4::look_at_rh(camera.position, camera.get_target(), camera.up);
        let proj = reverse_z_matrix()
            * Mat4::perspective_rh(camera.fov_y, camera.aspect, camera.znear, camera.zfar);

        let pos = camera.get_position();

        CameraRaw {
            view_proj: (proj * view).to_cols_array_2d(),
            view: view.to_cols_array_2d(),
            view_inv: view.transpose().to_cols_array_2d(),
            proj: proj.to_cols_array_2d(),
            proj_inv: proj.inverse().to_cols_array_2d(),
            camera_pos: [pos.x, pos.y, pos.z, 1.0],
        }
    }

    pub fn deproject_screen_to_world(&self, screen_coords: Vec3) -> Vec3 {
        let view = Mat4::look_at_rh(
            self.camera.position,
            self.camera.get_target(),
            self.camera.up,
        );
        let proj = Mat4::perspective_rh(
            self.camera.fov_y,
            self.camera.aspect,
            self.camera.znear,
            self.camera.zfar,
        );

        let result = (proj * view).inverse()
            * Vec4::new(
                screen_coords.x / (self.width as f32) * 2.0 - 1.0, // Clip space goes from -1 to 1, so transform there
                (screen_coords.y / (self.height as f32) * 2.0 - 1.0) * -1.0, // Clip space goes from -1 to 1, so transform there
                screen_coords.z,
                1.0,
            );
        result.xyz() / result.w
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraRaw {
    view_proj: [[f32; 4]; 4],
    view: [[f32; 4]; 4],
    view_inv: [[f32; 4]; 4],
    proj: [[f32; 4]; 4],
    proj_inv: [[f32; 4]; 4],
    camera_pos: [f32; 4],
}
