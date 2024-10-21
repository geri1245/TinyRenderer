use glam::{EulerRot, Quat, Vec2, Vec3};
use std::f32::consts::PI;
use std::time::Duration;
use winit::event::*;
use winit::keyboard::{KeyCode, PhysicalKey};

const REFERENCE_DIRECTION: Vec3 = Vec3::new(1.0, 0.0, 0.0);
const CAMERA_UP_VECTOR: Vec3 = Vec3::new(0 as f32, 1 as f32, 0 as f32);

const DEFAULT_FOV_Y: f32 = 45.0;
const DEFAULT_MOVEMENT_SENSITIVITY: f32 = 20.0;
const DEFAULT_MOUSE_LOOK_SENSITIVITY: f32 = 0.005;

pub enum CameraEvent {
    Motion((f64, f64)),
    Key(KeyEvent),
}

/// Contains the math part of the camera
/// For rendering-related things, see camera_controller

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Camera {
    pub position: Vec3,
    pub up: Vec3,
    pub aspect: f32,
    pub znear: f32,
    pub zfar: f32,
    pub orientation: (f32, f32, f32),
    pub fov_y: f32,

    look_sensitivity: Vec2,
    movement_sensitivity: Vec3,

    #[serde(skip_serializing)]
    #[serde(default)]
    current_speed_positive: Vec3,
    #[serde(skip_serializing)]
    #[serde(default)]
    current_speed_negative: Vec3,
}

impl Camera {
    pub fn new(width: u32, height: u32) -> Self {
        let eye: Vec3 = Vec3::new(-12.0, 10.0, 0.0);
        let target: Vec3 = Vec3::new(0.0, 0.0, 0.0);
        let view_dir = (target - eye).normalize();
        let rotation_quat = Quat::from_axis_angle(
            view_dir.cross(REFERENCE_DIRECTION).normalize(),
            -view_dir.angle_between(REFERENCE_DIRECTION),
        );
        // TODO: sort out this 3-tuple and probably use quaternions
        let orientation = rotation_quat.to_euler(EulerRot::ZYX);

        // TODO: calculate orientation properly. Now the camera can flip

        Self {
            position: eye,
            up: CAMERA_UP_VECTOR,
            aspect: width as f32 / height as f32,
            znear: 0.1,
            zfar: 300.0,
            orientation,
            look_sensitivity: Vec2::new(
                DEFAULT_MOUSE_LOOK_SENSITIVITY,
                DEFAULT_MOUSE_LOOK_SENSITIVITY,
            ),
            movement_sensitivity: Vec3::new(
                DEFAULT_MOVEMENT_SENSITIVITY,
                DEFAULT_MOVEMENT_SENSITIVITY,
                DEFAULT_MOVEMENT_SENSITIVITY,
            ),
            current_speed_positive: Vec3::ZERO,
            current_speed_negative: Vec3::ZERO,
            fov_y: DEFAULT_FOV_Y,
        }
    }

    pub fn get_position(&self) -> Vec3 {
        self.position
    }

    pub fn get_forward(&self) -> Vec3 {
        let pitch_rotation = Quat::from_rotation_y(self.orientation.0);
        let yaw_rotation = Quat::from_rotation_z(self.orientation.2);
        (pitch_rotation * yaw_rotation).mul_vec3(REFERENCE_DIRECTION)
    }

    pub fn get_right(&self) -> Vec3 {
        self.get_forward().cross(CAMERA_UP_VECTOR).normalize()
    }

    pub fn get_target(&self) -> Vec3 {
        self.position + self.get_forward()
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.aspect = width as f32 / height as f32;
    }

    fn handle_keyboard_event(&mut self, keyboard_event: &KeyEvent) {
        match keyboard_event.state {
            ElementState::Pressed => {
                if let PhysicalKey::Code(keycode) = keyboard_event.physical_key {
                    match keycode {
                        KeyCode::KeyW => self.current_speed_positive.z = 1.0,
                        KeyCode::KeyS => self.current_speed_negative.z = 1.0,
                        KeyCode::KeyA => self.current_speed_negative.x = 1.0,
                        KeyCode::KeyD => self.current_speed_positive.x = 1.0,
                        KeyCode::KeyQ => self.current_speed_positive.y = 1.0,
                        KeyCode::KeyE => self.current_speed_negative.y = 1.0,
                        _ => (),
                    }
                }
            }
            ElementState::Released => {
                if let PhysicalKey::Code(keycode) = keyboard_event.physical_key {
                    match keycode {
                        KeyCode::KeyW => self.current_speed_positive.z = 0.0,
                        KeyCode::KeyS => self.current_speed_negative.z = 0.0,
                        KeyCode::KeyA => self.current_speed_negative.x = 0.0,
                        KeyCode::KeyD => self.current_speed_positive.x = 0.0,
                        KeyCode::KeyQ => self.current_speed_positive.y = 0.0,
                        KeyCode::KeyE => self.current_speed_negative.y = 0.0,
                        _ => (),
                    }
                }
            }
        }
    }

    pub fn stop_movement(&mut self) {
        self.current_speed_negative = Vec3::ZERO;
        self.current_speed_positive = Vec3::ZERO;
    }

    pub fn process_event(&mut self, event: &CameraEvent) {
        match event {
            CameraEvent::Motion(delta) => self.rotate((delta.0 as f32, delta.1 as f32)),
            CameraEvent::Key(key_event) => self.handle_keyboard_event(key_event),
        }
    }

    pub fn update(&mut self, delta: Duration) {
        let current_speed = self.current_speed_positive - self.current_speed_negative;
        if current_speed == Vec3::ZERO {
            return;
        }

        let speed_norm = current_speed.normalize();
        let right = speed_norm.x * self.get_right();
        let up = speed_norm.y * CAMERA_UP_VECTOR;
        let forward = speed_norm.z * self.get_forward();

        let v = delta.as_secs_f32() * (right + up + forward) * self.movement_sensitivity;

        self.position += v;
    }

    fn rotate(&mut self, (delta_x, delta_y): (f32, f32)) {
        self.orientation.0 += self.look_sensitivity.x * -delta_x;
        self.orientation.2 += self.look_sensitivity.y * -delta_y;
        self.orientation.2 = self
            .orientation
            .2
            .clamp(-PI / 2.0 + 0.0001, PI / 2.0 - 0.0001);
    }
}
