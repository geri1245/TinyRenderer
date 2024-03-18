use std::f32::consts;

use glam::{Mat4, Quat, Vec3, Vec3Swizzles};

use crate::instance::SceneComponent;

const POINT_LIGHT_FAR_PLANE: f32 = 100.0;
const DIRECTIONAL_LIGHT_FAR_PLANE: f32 = 250.0;
const NEAR_PLANE: f32 = 0.1;
const DIRECTIONAL_LIGHT_PROJECTION_CUBE_SIZE: f32 = 10.0;

pub enum LightType {
    Directional(DirectionalLight),
    Point(PointLight),
}

pub struct LightComponent {
    color: Vec3,
    light: LightType,
}

#[derive(Debug)]
pub struct PointLight {
    pub transform: SceneComponent,
    pub color: Vec3,
    // In the final implementation this should radiate light in every direction
    pub target: Vec3,
    pub depth_texture: Vec<wgpu::TextureView>,
    far_plane: f32,
    near_plane: f32,
}

#[repr(C)]
#[derive(Debug)]
pub struct DirectionalLight {
    pub direction: Vec3,
    pub color: Vec3,
    pub depth_texture: wgpu::TextureView,
    far_plane: f32,
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
    far_plane_distance: f32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightRawSmall {
    light_view_proj: [[f32; 4]; 4],
    position_and_far_plane_distance: [f32; 4],
}

impl PointLight {
    pub fn new(
        depth_texture: Vec<wgpu::TextureView>,
        position: Vec3,
        color: Vec3,
        target: Vec3,
    ) -> Self {
        PointLight {
            transform: SceneComponent {
                position,
                rotation: Quat::IDENTITY,
                scale: Vec3::splat(0.2),
            },
            color,
            target,
            depth_texture,
            far_plane: POINT_LIGHT_FAR_PLANE,
            near_plane: NEAR_PLANE,
        }
    }

    pub fn get_viewprojs_raw(&self) -> Vec<LightRawSmall> {
        let proj =
            glam::Mat4::perspective_rh(consts::FRAC_PI_2, 1.0, self.near_plane, self.far_plane);

        const DIFF_AND_UP_VECTORS: [(Vec3, Vec3); 6] = [
            (Vec3::X, Vec3::Y),
            (Vec3::NEG_X, Vec3::Y),
            (Vec3::Y, Vec3::NEG_Z),
            (Vec3::NEG_Y, Vec3::Z),
            (Vec3::Z, Vec3::Y),
            (Vec3::NEG_Z, Vec3::Y),
        ];

        DIFF_AND_UP_VECTORS
            .iter()
            .map(|&(diff, up)| {
                let view = Mat4::look_at_rh(
                    self.transform.position.into(),
                    (self.transform.position + diff).into(),
                    up,
                );
                proj * view
            })
            .map(|view_proj| {
                let mut position_and_far_plane_distance = self.transform.position.xyzz();
                position_and_far_plane_distance.w = self.far_plane;
                LightRawSmall {
                    light_view_proj: view_proj.to_cols_array_2d(),
                    position_and_far_plane_distance: position_and_far_plane_distance.into(),
                }
            })
            .collect::<Vec<_>>()
    }

    pub fn to_raw(&self) -> LightRaw {
        let view = Mat4::look_at_rh(
            self.transform.position.into(),
            self.target.into(),
            Vec3::new(0.0_f32, 1.0, 0.0),
        );
        let proj =
            glam::Mat4::perspective_rh(consts::FRAC_PI_3, 1.0, self.near_plane, self.far_plane);
        let view_proj = proj * view;
        LightRaw {
            light_view_proj: view_proj.to_cols_array_2d(),
            position_or_direction: self.transform.position.into(),
            light_type: 1,
            color: self.color.into(),
            far_plane_distance: 100.0,
        }
    }
}

impl DirectionalLight {
    pub fn new(depth_texture: wgpu::TextureView, direction: Vec3, color: Vec3) -> Self {
        DirectionalLight {
            direction,
            color,
            depth_texture,
            far_plane: DIRECTIONAL_LIGHT_FAR_PLANE,
        }
    }

    pub fn to_raw(&self) -> LightRaw {
        let direction_vec = Vec3::from(self.direction);
        let right = direction_vec.cross(Vec3::new(0.0, 1.0, 0.0));
        // In case of directional lights, the eye is set to a number, so that when we are rendering shadows
        // with this viewproj matrix, then everything is hopefully inside of it
        let view = Mat4::look_at_rh(
            25.0 * -direction_vec,
            Vec3::ZERO,
            right.cross(direction_vec),
        );
        let proj: Mat4 = Mat4::orthographic_rh(
            0.0,
            DIRECTIONAL_LIGHT_PROJECTION_CUBE_SIZE,
            0.0,
            DIRECTIONAL_LIGHT_PROJECTION_CUBE_SIZE,
            NEAR_PLANE,
            self.far_plane,
        );
        let view_proj = proj * view;
        LightRaw {
            light_view_proj: view_proj.to_cols_array_2d(),
            position_or_direction: self.direction.into(),
            light_type: 2,
            color: self.color.into(),
            far_plane_distance: self.far_plane,
        }
    }
}
