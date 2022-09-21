use cgmath::{
    num_traits::clamp, ElementWise, Euler, InnerSpace, Point3, Quaternion, Rad, Rotation,
    Rotation3, Vector2, Vector3, Zero,
};
use std::f32::consts::PI;
use std::time::Duration;
use winit::event::*;

const REFERENCE_DIRECTION: Vector3<f32> = Vector3::new(1.0, 0.0, 0.0);
const CAMERA_UP_VECTOR: Vector3<f32> = Vector3::new(0 as f32, 1 as f32, 0 as f32);

const MOVEMENT_SENSITIVITY: f32 = 20.0;
const MOUSE_LOOK_SENSITIVITY: f32 = 0.005;

/// Contains only camera interactions, nothing rendering-related
pub struct Camera {
    pub position: cgmath::Point3<f32>,
    pub up: Vector3<f32>,
    pub aspect: f32,
    pub fovy: cgmath::Deg<f32>,
    pub znear: f32,
    pub zfar: f32,
    pub orientation: Euler<cgmath::Rad<f32>>,
    look_sensitivity: Vector2<f32>,
    current_speed_positive: Vector3<f32>,
    current_speed_negative: Vector3<f32>,
    movement_sensitivity: Vector3<f32>,
}

impl Camera {
    pub fn new(aspect_ratio: f32) -> Self {
        let eye: Point3<f32> = (-12.0, 10.0, 0.0).into();
        let target: Point3<f32> = (0.0, 0.0, 0.0).into();
        let view_dir = (target - eye).normalize();
        let rotation_quat = Quaternion::from_axis_angle(
            view_dir.cross(REFERENCE_DIRECTION).normalize(),
            -view_dir.angle(REFERENCE_DIRECTION),
        );
        let orientation = Euler::from(rotation_quat);
        // TODO: calculate orientation properly. Now the camera can flip

        Self {
            position: eye,
            up: CAMERA_UP_VECTOR,
            aspect: aspect_ratio,
            fovy: cgmath::Deg(45.0),
            znear: 0.1,
            zfar: 100.0,
            orientation,
            look_sensitivity: cgmath::Vector2::new(MOUSE_LOOK_SENSITIVITY, MOUSE_LOOK_SENSITIVITY),
            movement_sensitivity: Vector3::new(
                MOVEMENT_SENSITIVITY,
                MOVEMENT_SENSITIVITY,
                MOVEMENT_SENSITIVITY,
            ),
            current_speed_positive: Vector3::<f32>::zero(),
            current_speed_negative: Vector3::<f32>::zero(),
        }
    }

    pub fn get_position(&self) -> cgmath::Point3<f32> {
        self.position
    }

    pub fn get_forward(&self) -> Vector3<f32> {
        let pitch_rotation = Quaternion::from_angle_y(self.orientation.x);
        let yaw_rotation = Quaternion::from_angle_z(self.orientation.z);
        (pitch_rotation * yaw_rotation).rotate_vector(REFERENCE_DIRECTION)
    }

    pub fn get_right(&self) -> Vector3<f32> {
        self.get_forward().cross(CAMERA_UP_VECTOR).normalize()
    }

    pub fn get_target(&self) -> Point3<f32> {
        self.position + self.get_forward()
    }

    pub fn resize(&mut self, aspect: f32) {
        self.aspect = aspect;
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

    pub fn stop_movement(&mut self) {
        self.current_speed_negative = Vector3::zero();
        self.current_speed_positive = Vector3::zero();
    }

    pub fn process_device_events(&mut self, event: DeviceEvent) {
        match event {
            DeviceEvent::MouseMotion { delta } => {
                self.rotate((delta.0 as f32, delta.1 as f32));
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

        self.position += v;
    }

    fn rotate(&mut self, (delta_x, delta_y): (f32, f32)) {
        self.orientation.x += Rad(self.look_sensitivity.x * -delta_x);
        self.orientation.z += Rad(self.look_sensitivity.y * -delta_y);
        self.orientation.z = clamp(
            self.orientation.z,
            Rad(-PI / 2.0 + 0.0001),
            Rad(PI / 2.0 - 0.0001),
        );
    }
}
