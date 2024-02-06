use std::f32::consts;

use glam::{Mat4, Vec3};

#[derive(Debug, Copy, Clone)]
pub struct PointLight {
    pub position: [f32; 3],
    pub color: [f32; 3],
    // Only used while real implementation is in progress
    // In the final implementation this should radiate light in every direction
    pub target: [f32; 3],
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DirectionalLight {
    pub direction: [f32; 3],
    pub color: [f32; 3],
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightRaw {
    pub light_view_proj: [[f32; 4]; 4],
    pub position_or_direction: [f32; 3],
    // 1 means point light
    // 2 means directional light
    pub light_type: u32,
    pub color: [f32; 3],
    // Due to uniforms requiring 16 byte (4 float) spacing, we need to use a padding field here
    _padding2: u32,
}

impl PointLight {
    pub fn to_raw(&self) -> LightRaw {
        let view = Mat4::look_at_rh(
            self.position.into(),
            self.target.into(),
            Vec3::new(0.0_f32, 1.0, 0.0),
        );
        let proj = glam::Mat4::perspective_rh(consts::FRAC_PI_3, 1.0, 1.0, 100.0);
        let view_proj = proj * view;
        LightRaw {
            light_view_proj: view_proj.to_cols_array_2d(),
            position_or_direction: self.position,
            light_type: 1,
            color: self.color,
            _padding2: 0,
        }
    }
}

impl DirectionalLight {
    pub fn to_raw(&self) -> LightRaw {
        let direction_vec = Vec3::from(self.direction);
        let view = Mat4::look_at_rh(-direction_vec, Vec3::ZERO, Vec3::new(0.0_f32, 1.0, 0.0));
        let proj = glam::Mat4::perspective_rh(consts::FRAC_PI_3, 1.0, 1.0, 100.0);
        let view_proj = proj * view;
        LightRaw {
            light_view_proj: view_proj.to_cols_array_2d(),
            position_or_direction: self.direction,
            light_type: 2,
            color: self.color,
            _padding2: 0,
        }
    }
}
