use std::f32::consts;

use glam::{Mat4, Quat, Vec3, Vec3Swizzles};

use crate::{instance::SceneComponent, serde_helpers::SerdeVec3Proxy};

const POINT_LIGHT_FAR_PLANE: f32 = 100.0;
const DIRECTIONAL_LIGHT_FAR_PLANE: f32 = 250.0;
const NEAR_PLANE: f32 = 0.1;
const DIRECTIONAL_LIGHT_PROJECTION_CUBE_SCALE: f32 = 40.0;
const DIRECTIONAL_LIGHT_PROJECTION_CUBE_OFFSET: f32 =
    -DIRECTIONAL_LIGHT_PROJECTION_CUBE_SCALE / 2.0;

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub enum Light {
    Point(PointLight),
    Directional(DirectionalLight),
}

#[derive(Debug)]
pub struct CommonLightParams {
    far_plane: f32,
    near_plane: f32,
}

#[derive(Debug, Copy, Clone, serde::Serialize)]
pub struct PointLight {
    pub transform: SceneComponent,
    #[serde(with = "SerdeVec3Proxy")]
    pub color: Vec3,
}

pub struct PointLightRenderData {
    pub light: PointLight,
    pub depth_texture_index: usize,
    light_params: CommonLightParams,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, serde::Serialize)]
pub struct DirectionalLight {
    #[serde(with = "SerdeVec3Proxy")]
    pub direction: Vec3,
    #[serde(with = "SerdeVec3Proxy")]
    pub color: Vec3,
}

pub struct DirectionalLightRenderData {
    pub light: DirectionalLight,
    pub depth_texture: wgpu::TextureView,
    light_params: CommonLightParams,
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
    pub fn new(position: Vec3, color: Vec3) -> Self {
        Self {
            color,
            transform: SceneComponent {
                position,
                rotation: Quat::IDENTITY,
                scale: Vec3::splat(0.2),
            },
        }
    }
}

impl PointLightRenderData {
    pub fn new(point_light: PointLight, depth_texture_index: usize) -> Self {
        PointLightRenderData {
            light: point_light,
            depth_texture_index,
            light_params: CommonLightParams {
                far_plane: POINT_LIGHT_FAR_PLANE,
                near_plane: NEAR_PLANE,
            },
        }
    }

    pub fn get_viewprojs_raw(&self) -> Vec<LightRawSmall> {
        let proj = glam::Mat4::perspective_rh(
            consts::FRAC_PI_2,
            1.0,
            self.light_params.near_plane,
            self.light_params.far_plane,
        );

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
                    self.light.transform.position.into(),
                    (self.light.transform.position + diff).into(),
                    up,
                );
                proj * view
            })
            .map(|view_proj| {
                let mut position_and_far_plane_distance = self.light.transform.position.xyzz();
                position_and_far_plane_distance.w = self.light_params.far_plane;
                LightRawSmall {
                    light_view_proj: view_proj.to_cols_array_2d(),
                    position_and_far_plane_distance: position_and_far_plane_distance.into(),
                }
            })
            .collect::<Vec<_>>()
    }

    pub fn to_raw(&self) -> LightRaw {
        let view = Mat4::look_at_rh(
            self.light.transform.position.into(),
            Vec3::ZERO,
            Vec3::new(0.0_f32, 1.0, 0.0),
        );
        let proj = glam::Mat4::perspective_rh(
            consts::FRAC_PI_3,
            1.0,
            self.light_params.near_plane,
            self.light_params.far_plane,
        );
        let view_proj = proj * view;
        LightRaw {
            light_view_proj: view_proj.to_cols_array_2d(),
            position_or_direction: self.light.transform.position.into(),
            light_type: 1,
            color: self.light.color.into(),
            far_plane_distance: 100.0,
        }
    }
}

impl DirectionalLightRenderData {
    pub fn new(light: &DirectionalLight, depth_texture: wgpu::TextureView) -> Self {
        Self {
            depth_texture: depth_texture,
            light: light.clone(),
            light_params: CommonLightParams {
                far_plane: DIRECTIONAL_LIGHT_FAR_PLANE,
                near_plane: NEAR_PLANE,
            },
        }
    }

    pub fn to_raw(&self) -> LightRaw {
        let direction_vec = Vec3::from(self.light.direction);
        let right = direction_vec.cross(Vec3::new(1.0, 0.0, 0.0));
        // In case of directional lights, the eye is set to a number, so that when we are rendering shadows
        // with this viewproj matrix, then everything is hopefully inside of it
        let view = Mat4::look_at_rh(
            25.0 * -direction_vec,
            Vec3::ZERO,
            right.cross(direction_vec),
        );
        let proj: Mat4 = Mat4::orthographic_rh(
            DIRECTIONAL_LIGHT_PROJECTION_CUBE_OFFSET,
            DIRECTIONAL_LIGHT_PROJECTION_CUBE_SCALE,
            DIRECTIONAL_LIGHT_PROJECTION_CUBE_OFFSET,
            DIRECTIONAL_LIGHT_PROJECTION_CUBE_SCALE,
            NEAR_PLANE,
            self.light_params.far_plane,
        );
        let view_proj = proj * view;
        LightRaw {
            light_view_proj: view_proj.to_cols_array_2d(),
            position_or_direction: self.light.direction.into(),
            light_type: 2,
            color: self.light.color.into(),
            far_plane_distance: self.light_params.far_plane,
        }
    }

    pub fn to_raw_small(&self) -> LightRawSmall {
        let direction_vec = Vec3::from(self.light.direction);
        let right = direction_vec.cross(Vec3::new(1.0, 0.0, 0.0));
        // In case of directional lights, the eye is set to a number, so that when we are rendering shadows
        // with this viewproj matrix, then everything is hopefully inside of it
        let view = Mat4::look_at_rh(
            30.0 * -direction_vec,
            Vec3::ZERO,
            right.cross(direction_vec),
        );
        let proj: Mat4 = Mat4::orthographic_rh(
            DIRECTIONAL_LIGHT_PROJECTION_CUBE_OFFSET,
            DIRECTIONAL_LIGHT_PROJECTION_CUBE_SCALE,
            DIRECTIONAL_LIGHT_PROJECTION_CUBE_OFFSET,
            DIRECTIONAL_LIGHT_PROJECTION_CUBE_SCALE,
            NEAR_PLANE,
            self.light_params.far_plane,
        );
        let view_proj = proj * view;

        let mut position_and_far_plane_distance = self.light.direction.xyzz();
        position_and_far_plane_distance.z = self.light_params.far_plane;

        LightRawSmall {
            light_view_proj: view_proj.to_cols_array_2d(),
            position_and_far_plane_distance: position_and_far_plane_distance.into(),
        }
    }
}
