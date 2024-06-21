use core::mem;

use glam::{Mat3, Mat4, Quat, Vec3};

use crate::{
    buffer_content::BufferContent,
    serde_helpers::{serialize_quat, SerdeVec3Proxy},
};

#[derive(Debug, Copy, Clone, serde::Serialize)]
pub struct SceneComponent {
    #[serde(with = "SerdeVec3Proxy")]
    pub position: Vec3,
    #[serde(with = "SerdeVec3Proxy")]
    pub scale: Vec3,
    #[serde(serialize_with = "serialize_quat")]
    pub rotation: Quat,
}

// impl serde::Serialize for SceneComponent {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: serde::Serializer,
//     {
//         let serializable_self = self.to_serializable();
//         let mut struct_serializer = serializer.serialize_struct("SerializableSceneComponent", 3)?;
//         struct_serializer.serialize_field("position", &serializable_self.position)?;
//         struct_serializer.serialize_field("rotation", &serializable_self.rotation)?;
//         struct_serializer.serialize_field("scale", &serializable_self.scale)?;
//         struct_serializer.end()
//     }
// }

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SceneComponentRaw {
    pub model_matrix: [[f32; 4]; 4],
    pub rotation_only_matrix: [[f32; 3]; 3],
}

impl SceneComponent {
    pub fn to_raw(&self) -> SceneComponentRaw {
        SceneComponentRaw {
            model_matrix: Mat4::from_scale_rotation_translation(
                self.scale,
                self.rotation,
                self.position,
            )
            .to_cols_array_2d(),
            // Instead of the inverse transpose, we can just pass the rotation matrix
            // As non-uniform scaling is not supported, this is fine
            rotation_only_matrix: Mat3::from_quat(self.rotation).to_cols_array_2d(),
        }
    }

    // pub fn to_serializable(&self) -> SerializableSceneComponent {
    //     SerializableSceneComponent {
    //         position: SerdeVec3Proxy::from_vec3(&self.position),
    //         scale: SerdeVec3Proxy::from_vec3(&self.scale),
    //         rotation: self.rotation.to_array(),
    //     }
    // }

    // pub fn from_serializable(&self) -> SerializableSceneComponent {
    //     SerializableSceneComponent {
    //         position: SerdeVec3Proxy::from_vec3(&self.position),
    //         scale: SerdeVec3Proxy::from_vec3(&self.scale),
    //         rotation: self.rotation.to_array(),
    //     }
    // }
}

impl BufferContent for SceneComponentRaw {
    fn buffer_layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<SceneComponentRaw>() as wgpu::BufferAddress,
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
            ],
        }
    }
}
