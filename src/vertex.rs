use crate::buffer_content::BufferContent;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VertexRawWithTangents {
    pub position: [f32; 3],
    pub tex_coord: [f32; 2],
    pub normal: [f32; 3],
    pub tangent: [f32; 3],
    pub bitangent: [f32; 3],
}

impl BufferContent for VertexRawWithTangents {
    fn buffer_layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem::size_of;
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<VertexRawWithTangents>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            // attributes: &vertex_attr_array![
            //     0 => Float32x3, // Position
            //     1 => Float32x2, // Texture coordinates
            //     2 => Float32x3, // Normal
            //     3 => Float32x3, // Tangent
            //     4 => Float32x3, // Bitangent
            // ],
            attributes: &[
                // Position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Tex coords
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // Normals
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 5]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Tangents
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Bitangents
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 11]>() as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}
