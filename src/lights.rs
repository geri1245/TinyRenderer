use std::f32::consts;

use glam::{Mat4, Vec3};

#[derive(Debug)]
pub struct PointLight {
    pub position: Vec3,
    pub color: Vec3,
    // In the final implementation this should radiate light in every direction
    pub target: Vec3,
    pub depth_texture: wgpu::TextureView,
}

#[repr(C)]
#[derive(Debug)]
pub struct DirectionalLight {
    pub direction: Vec3,
    pub color: Vec3,
    pub depth_texture: wgpu::TextureView,
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
    _padding: u32,
}

impl PointLight {
    pub fn new(
        depth_texture: wgpu::TextureView,
        position: Vec3,
        color: Vec3,
        target: Vec3,
    ) -> Self {
        PointLight {
            position,
            color,
            target,
            depth_texture,
        }
    }

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
            position_or_direction: self.position.into(),
            light_type: 1,
            color: self.color.into(),
            _padding: 0,
        }
    }
}

impl DirectionalLight {
    pub fn new(depth_texture: wgpu::TextureView, direction: Vec3, color: Vec3) -> Self {
        DirectionalLight {
            direction,
            color,
            depth_texture,
        }
    }

    pub fn to_raw(&self) -> LightRaw {
        let direction_vec = Vec3::from(self.direction);
        // In case of directional lights, the eye is set to a number, so that when we are rendering shadows
        // with this viewproj matrix, then everything is hopefully inside of it
        let view = Mat4::look_at_rh(
            50.0 * -direction_vec,
            Vec3::ZERO,
            Vec3::new(0.0_f32, 1.0, 0.0),
        );
        let proj = glam::Mat4::perspective_rh(consts::FRAC_PI_3, 1.0, 1.0, 100.0);
        let view_proj = proj * view;
        LightRaw {
            light_view_proj: view_proj.to_cols_array_2d(),
            position_or_direction: self.direction.into(),
            light_type: 2,
            color: self.color.into(),
            _padding: 0,
        }
    }
}
