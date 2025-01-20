use std::f32::consts;

use glam::{Mat4, Vec3, Vec3Swizzles};

use crate::{
    components::TransformComponent,
    math::reverse_z_matrix,
    world_object::{OmnipresentObject, WorldObject},
};

/// These are used on the shader side
const POINT_LIGHT_TYPE_RAW: u32 = 1;
const DIRECTIONAL_LIGHT_TYPE_RAW: u32 = 2;

const POINT_LIGHT_FAR_PLANE: f32 = 100.0;
const DIRECTIONAL_LIGHT_FAR_PLANE: f32 = 250.0;
const NEAR_PLANE: f32 = 0.1;
/// The width and the depth of the orthographic projection used by the directional lights
const DIRECTIONAL_LIGHT_PROJECTION_CUBE_SIZE: f32 = 100.0;
/// How much to offset the directional light ortographic projection
const DIRECTIONAL_LIGHT_PROJECTION_CUBE_OFFSET: f32 = -DIRECTIONAL_LIGHT_PROJECTION_CUBE_SIZE / 2.0;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy)]
pub enum Light {
    Point(PointLightRenderData),
    Directional(DirectionalLight),
}

impl Light {
    pub fn from_world_object(world_object: &WorldObject) -> Option<Self> {
        if let Some(light_component) = world_object.get_light_component() {
            let light = Light::Point(PointLightRenderData {
                transform: world_object.transform,
                color: light_component.light.color,
            });
            Some(light)
        } else {
            None
        }
    }

    pub fn from_omnipresent_object(omnipresent_object: &OmnipresentObject) -> Option<Self> {
        if let Some(directional_light) = omnipresent_object.get_light_component() {
            let light = Light::Directional(directional_light.clone());
            Some(light)
        } else {
            None
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy)]
pub enum LightNew {
    Point(PointLight),
    Directional(DirectionalLight),
}

#[derive(Debug)]
pub struct CommonLightParams {
    far_plane: f32,
    near_plane: f32,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Copy, Clone)]
pub struct PointLight {
    pub color: Vec3,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Copy, Clone)]
pub struct PointLightRenderData {
    pub transform: TransformComponent,
    pub color: Vec3,
}

pub struct PointLightData {
    /// Standard parameters for the light
    pub light: PointLightRenderData,
    /// Which depth texture view to render into from the texture array
    pub depth_texture_index: usize,
    light_params: CommonLightParams,
}

#[repr(C)]
#[derive(serde::Serialize, serde::Deserialize, Debug, Copy, Clone)]
pub struct DirectionalLight {
    pub direction: Vec3,
    pub color: Vec3,
}

pub struct DirectionalLightData {
    pub light: DirectionalLight,
    pub depth_texture_index: usize,
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
    // Due to uniforms requiring 16 byte spacing, we need to use a padding field here
    far_plane_distance: f32,
    depth_texture_index: u32,
    padding: [f32; 3],
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightRawSmall {
    light_view_proj: [[f32; 4]; 4],
    position_and_far_plane_distance: [f32; 4],
}

impl PointLightData {
    pub fn new(point_light: PointLightRenderData, depth_texture_index: usize) -> Self {
        PointLightData {
            light: point_light,
            depth_texture_index,
            light_params: CommonLightParams {
                far_plane: POINT_LIGHT_FAR_PLANE,
                near_plane: NEAR_PLANE,
            },
        }
    }

    pub fn get_viewprojs_raw(&self) -> Vec<LightRawSmall> {
        let proj = reverse_z_matrix()
            * glam::Mat4::perspective_rh(
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
                    self.light.transform.get_position().into(),
                    (self.light.transform.get_position() + diff).into(),
                    up,
                );
                proj * view
            })
            .map(|view_proj| {
                let mut position_and_far_plane_distance =
                    self.light.transform.get_position().xyzz();
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
            self.light.transform.get_position().into(),
            Vec3::ZERO,
            Vec3::new(0.0_f32, 1.0, 0.0),
        );
        let proj = reverse_z_matrix()
            * glam::Mat4::perspective_rh(
                consts::FRAC_PI_3,
                1.0,
                self.light_params.near_plane,
                self.light_params.far_plane,
            );
        let view_proj = proj * view;
        LightRaw {
            light_view_proj: view_proj.to_cols_array_2d(),
            position_or_direction: self.light.transform.get_position().into(),
            light_type: POINT_LIGHT_TYPE_RAW,
            color: self.light.color.into(),
            far_plane_distance: 100.0,
            depth_texture_index: self.depth_texture_index as u32,
            padding: [0.0; 3],
        }
    }
}

impl DirectionalLightData {
    pub fn new(light: &DirectionalLight, depth_texture_index: usize) -> Self {
        Self {
            depth_texture_index,
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
            DIRECTIONAL_LIGHT_PROJECTION_CUBE_SIZE,
            DIRECTIONAL_LIGHT_PROJECTION_CUBE_OFFSET,
            DIRECTIONAL_LIGHT_PROJECTION_CUBE_SIZE,
            self.light_params.near_plane,
            self.light_params.far_plane,
        );
        let view_proj = proj * view;
        LightRaw {
            light_view_proj: view_proj.to_cols_array_2d(),
            position_or_direction: self.light.direction.into(),
            light_type: DIRECTIONAL_LIGHT_TYPE_RAW,
            color: self.light.color.into(),
            far_plane_distance: self.light_params.far_plane,
            depth_texture_index: self.depth_texture_index as u32,
            padding: [0.0; 3],
        }
    }

    pub fn get_viewprojs_raw(&self) -> LightRawSmall {
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
            DIRECTIONAL_LIGHT_PROJECTION_CUBE_SIZE,
            DIRECTIONAL_LIGHT_PROJECTION_CUBE_OFFSET,
            DIRECTIONAL_LIGHT_PROJECTION_CUBE_SIZE,
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
