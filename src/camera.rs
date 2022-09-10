#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.0,
    0.0, 0.0, 0.5, 1.0,
);

use cgmath::Vector3 as Vec3;

const REFERENCE_UP: f32 = PI / 2.0;
// const REFERENCE_DIRECTION: Vec3<f32> = Vec3::new(1.0, 0.0, 0.0);

use cgmath::{ElementWise, InnerSpace, Point3, Zero};
use winit::event::*;

use std::f32::consts::PI;
use std::time::Duration;

const CAMERA_UP_VECTOR: Vec3<f32> = Vec3::new(0 as f32, 1 as f32, 0 as f32);

const MOVEMENT_SENSITIVITY: f32 = 20.0;
const MOUSE_LOOK_SENSITIVITY: f32 = 0.005;

fn spherical_to_cartesian((phi, theta): (f32, f32)) -> Vec3<f32> {
    Vec3::new(
        theta.sin() * phi.cos(),
        theta.cos(),
        theta.sin() * phi.sin(),
    )
}

// Result is given in order of (phi, theta)
// fn cartesian_to_spherical(
//     target: Vec3<f32>,
//     origin: Vec3<f32>,
// ) -> (cgmath::Rad<f32>, cgmath::Rad<f32>) {
//     let coords = target - origin;
//     let inclination: cgmath::Rad<f32> = cgmath::Angle::acos(coords.y / coords.magnitude());
//     let azimuth: cgmath::Rad<f32> = cgmath::Angle::atan2(coords.z, coords.x);
//     (inclination, azimuth)
// }

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraRaw {
    view_proj: [[f32; 4]; 4],
    camera_pos: [f32; 4],
}

pub struct CameraController {
    eye: cgmath::Point3<f32>,
    up: cgmath::Vector3<f32>,
    aspect: f32,
    fovy: cgmath::Deg<f32>,
    znear: f32,
    zfar: f32,
    look_sensitivity: cgmath::Vector2<f32>,
    orientation: (f32, f32),
    current_speed_positive: Vec3<f32>,
    current_speed_negative: Vec3<f32>,
    movement_sensitivity: Vec3<f32>,
    is_rotation_enabled: bool,
}

impl CameraController {
    pub fn new(aspect_ratio: f32) -> Self {
        let eye: cgmath::Point3<f32> = (2.0, 5.0, 0.0).into();
        let target = Vec3::<f32>::zero();

        let phi: cgmath::Rad<f32> = cgmath::Angle::atan2(eye.y - target.y, eye.x - target.x);

        Self {
            eye,
            up: CAMERA_UP_VECTOR,
            aspect: aspect_ratio,
            fovy: cgmath::Deg(45.0),
            znear: 0.1,
            zfar: 100.0,
            orientation: (0.0, REFERENCE_UP + phi.0),
            look_sensitivity: cgmath::Vector2::new(MOUSE_LOOK_SENSITIVITY, MOUSE_LOOK_SENSITIVITY),
            movement_sensitivity: Vec3::new(
                MOVEMENT_SENSITIVITY,
                MOVEMENT_SENSITIVITY,
                MOVEMENT_SENSITIVITY,
            ),
            current_speed_positive: Vec3::<f32>::zero(),
            current_speed_negative: Vec3::<f32>::zero(),
            is_rotation_enabled: false,
        }
    }

    pub fn to_raw(&self) -> CameraRaw {
        let view = cgmath::Matrix4::look_at_rh(self.eye, self.get_target(), self.up);
        let proj = cgmath::perspective(self.fovy, self.aspect, self.znear, self.zfar);
        return CameraRaw {
            view_proj: (OPENGL_TO_WGPU_MATRIX * proj * view).into(),
            camera_pos: self.get_position().to_homogeneous().into(),
        };
    }

    pub fn get_position(&self) -> cgmath::Point3<f32> {
        self.eye
    }

    pub fn get_forward(&self) -> Vec3<f32> {
        spherical_to_cartesian(self.orientation)
    }

    fn get_right(&self) -> Vec3<f32> {
        self.get_forward().cross(CAMERA_UP_VECTOR).normalize()
    }

    fn get_target(&self) -> Point3<f32> {
        self.eye + self.get_forward()
    }

    pub fn resize(&mut self, aspect: f32) {
        self.aspect = aspect;
    }

    pub fn set_is_camera_rotation_enabled(&mut self, is_enabled: bool) {
        self.is_rotation_enabled = is_enabled;
    }

    fn handle_keyboard_event(&mut self, keyboard_event: &KeyboardInput) {
        match keyboard_event.state {
            ElementState::Pressed => {
                if let Some(keycode) = keyboard_event.virtual_keycode {
                    match keycode {
                        VirtualKeyCode::W => self.current_speed_positive.z = 1.0,
                        VirtualKeyCode::S => self.current_speed_negative.z = 1.0,
                        VirtualKeyCode::A => self.current_speed_negative.x = 1.0,
                        VirtualKeyCode::D => self.current_speed_positive.x = 1.0,
                        VirtualKeyCode::Q => self.current_speed_positive.y = 1.0,
                        VirtualKeyCode::E => self.current_speed_negative.y = 1.0,
                        _ => (),
                    }
                }
            }
            ElementState::Released => {
                if let Some(keycode) = keyboard_event.virtual_keycode {
                    match keycode {
                        VirtualKeyCode::W => self.current_speed_positive.z = 0.0,
                        VirtualKeyCode::S => self.current_speed_negative.z = 0.0,
                        VirtualKeyCode::A => self.current_speed_negative.x = 0.0,
                        VirtualKeyCode::D => self.current_speed_positive.x = 0.0,
                        VirtualKeyCode::Q => self.current_speed_positive.y = 0.0,
                        VirtualKeyCode::E => self.current_speed_negative.y = 0.0,
                        _ => (),
                    }
                }
            }
        }
    }

    pub fn process_device_events(&mut self, event: DeviceEvent) {
        match event {
            DeviceEvent::MouseMotion { delta } => {
                if self.is_rotation_enabled {
                    self.rotate(delta);
                }
            }
            DeviceEvent::Key(keyboard_input) => {
                self.handle_keyboard_event(&keyboard_input);
            }
            _ => (),
        }
    }

    pub fn update(&mut self, delta: Duration) {
        let current_speed = self.current_speed_positive - self.current_speed_negative;
        if current_speed.is_zero() {
            return;
        }

        let speed_norm = current_speed.normalize();
        let right = speed_norm.x * self.get_right();
        let up = speed_norm.y * CAMERA_UP_VECTOR;
        let forward = speed_norm.z * self.get_forward();

        let v = delta.as_secs_f32()
            * (right + up + forward).mul_element_wise(self.movement_sensitivity);

        self.eye += v;
    }

    fn rotate(&mut self, (delta_x, delta_y): (f64, f64)) {
        let (yaw, pitch) = self.orientation;
        let new_yaw = yaw + self.look_sensitivity.x * delta_x as f32;
        let new_pitch = (pitch + self.look_sensitivity.y * delta_y as f32).clamp(0.1, PI - 0.1);

        self.orientation = (new_yaw, new_pitch);
    }
}
