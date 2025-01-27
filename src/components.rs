use std::mem;

use glam::{Mat3, Mat4, Quat, Vec3};

use crate::{
    lights::{DirectionalLight, PointLight},
    material::PbrMaterialDescriptor,
    model::{MeshDescriptor, ModelDescriptor, ModelRenderingOptions},
};

use crate::buffer_content::BufferContent;

#[derive(
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Copy,
    Clone,
    ui_item_derive::UiDisplayable,
    ui_item_derive::UiSettableNew,
)]
pub struct TransformComponent {
    position: Vec3,
    scale: Vec3,
    rotation: Quat,

    #[serde(skip_serializing)]
    #[serde(default)]
    #[ui_param(skip)]
    #[ui_set(skip)]
    pub is_transform_dirty: bool,
}

impl Default for TransformComponent {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            scale: Vec3::ONE,
            rotation: Quat::IDENTITY,

            is_transform_dirty: false,
        }
    }
}

impl TransformComponent {
    pub fn new(position: Vec3, scale: Vec3, rotation: Quat) -> Self {
        Self {
            position,
            scale,
            rotation,
            is_transform_dirty: false,
        }
    }

    pub fn from_position(position: Vec3) -> Self {
        Self {
            position,
            ..Default::default()
        }
    }

    pub fn get_position(&self) -> Vec3 {
        self.position
    }

    pub fn set_location(&mut self, new_position: Vec3) {
        self.position = new_position;
        self.is_transform_dirty = true;
    }

    pub fn set_scale(&mut self, new_scale: f32) {
        self.scale = Vec3::splat(new_scale);
        self.is_transform_dirty = true;
    }

    pub fn to_raw(&self, object_id: u32) -> TransformComponentRaw {
        TransformComponentRaw {
            model_matrix: Mat4::from_scale_rotation_translation(
                self.scale,
                self.rotation,
                self.position,
            )
            .to_cols_array_2d(),
            // Instead of the inverse transpose, we can just pass the rotation matrix
            // As non-uniform scaling is not supported, this is fine
            rotation_only_matrix: Mat3::from_quat(self.rotation).to_cols_array_2d(),
            object_id,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TransformComponentRaw {
    pub model_matrix: [[f32; 4]; 4],
    pub rotation_only_matrix: [[f32; 3]; 3],
    pub object_id: u32,
}

impl BufferContent for TransformComponentRaw {
    fn buffer_layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<TransformComponentRaw>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            // We pass matrices to the shader column-by-column. We will reassemble it in the shader
            attributes: &[
                // Model matrix
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // Normal vectors
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 16]>() as wgpu::BufferAddress,
                    shader_location: 9,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 19]>() as wgpu::BufferAddress,
                    shader_location: 10,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 22]>() as wgpu::BufferAddress,
                    shader_location: 11,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 25]>() as wgpu::BufferAddress,
                    shader_location: 12,
                    format: wgpu::VertexFormat::Uint32,
                },
            ],
        }
    }
}

#[derive(
    Default,
    Debug,
    Clone,
    serde::Serialize,
    serde::Deserialize,
    ui_item_derive::UiDisplayable,
    ui_item_derive::UiSettableNew,
)]
pub struct RenderableComponent {
    pub model_descriptor: ModelDescriptor,
    pub rendering_options: ModelRenderingOptions,

    #[serde(skip_serializing)]
    #[serde(default)]
    #[ui_set(skip)]
    #[ui_param(skip)]
    pub is_transient: bool,

    #[serde(skip_serializing)]
    #[serde(default)]
    #[ui_set(skip)]
    #[ui_param(skip)]
    pub is_material_dirty: bool,
}

impl RenderableComponent {
    pub fn new(
        mesh_descriptor: MeshDescriptor,
        material_descriptor: PbrMaterialDescriptor,
        rendering_options: ModelRenderingOptions,
        is_transient: bool,
    ) -> Self {
        Self {
            model_descriptor: ModelDescriptor {
                mesh_descriptor,
                material_descriptor,
            },
            rendering_options,
            is_material_dirty: false,
            is_transient,
        }
    }

    pub fn update_material(&mut self, new_material: PbrMaterialDescriptor) {
        self.model_descriptor.material_descriptor = new_material;
        self.is_material_dirty = true;
    }
}

/// Can be extended to work as a spotlight as well
#[derive(
    Default,
    Debug,
    Clone,
    serde::Serialize,
    serde::Deserialize,
    ui_item_derive::UiDisplayable,
    ui_item_derive::UiSettableNew,
)]
pub struct LightObjectComponent {
    pub light: PointLight,
}

#[derive(
    Debug,
    Clone,
    serde::Serialize,
    serde::Deserialize,
    ui_item_derive::UiDisplayable,
    ui_item_derive::UiSettableNew,
)]
pub enum SceneComponentType {
    LightObject(LightObjectComponent),
    Renderable(RenderableComponent),
}

impl SceneComponentType {
    pub fn reset_dirty_state(&mut self) {
        match self {
            SceneComponentType::LightObject(_light_object_component) => {}
            SceneComponentType::Renderable(renderable_component) => {
                renderable_component.is_material_dirty = false;
            }
        }
    }

    pub fn is_transient(&self) -> bool {
        match self {
            SceneComponentType::LightObject(_light_object_component) => false,
            SceneComponentType::Renderable(renderable_component) => {
                renderable_component.is_transient
            }
        }
    }
}

/// A component that has a transform, so is "somewhere" in the world
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SceneComponent {
    transform: TransformComponent,
    inner_component: SceneComponentType,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum OmnipresentComponentType {
    DirectionalLight(DirectionalLight),
}

impl OmnipresentComponentType {
    pub fn reset_dirty_state(&mut self) {}
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ComponentType {
    Scene(SceneComponent),
    Omnipresent(OmnipresentComponentType),
}
