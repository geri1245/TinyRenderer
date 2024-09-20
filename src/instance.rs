use core::mem;

use glam::{Mat3, Mat4, Quat, Vec3};

use crate::buffer_content::BufferContent;

#[derive(serde::Serialize, serde::Deserialize, Debug, Copy, Clone)]
pub struct TransformComponent {
    pub position: Vec3,
    pub scale: Vec3,
    pub rotation: Quat,
}

impl Default for TransformComponent {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            scale: Vec3::ONE,
            rotation: Quat::IDENTITY,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SceneComponentRaw {
    pub model_matrix: [[f32; 4]; 4],
    pub rotation_only_matrix: [[f32; 3]; 3],
    pub object_id: u32,
}

impl TransformComponent {
    pub fn to_raw(&self, object_id: u32) -> SceneComponentRaw {
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
            object_id,
        }
    }
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
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 25]>() as wgpu::BufferAddress,
                    shader_location: 12,
                    format: wgpu::VertexFormat::Uint32,
                },
            ],
        }
    }
}
