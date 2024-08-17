use crate::model::{BufferWithLength, Primitive};
use crate::vertex::VertexRawWithTangents;
use wgpu::util::DeviceExt;

const SQUARE_VERTICES: &'static [VertexRawWithTangents] = &[
    VertexRawWithTangents {
        position: [-0.5, 0.0, -0.5],
        tex_coord: [0.0, 0.0],
        normal: [0.0, 1.0, 0.0],
        tangent: [0.0, 1.0, 0.0],
        bitangent: [0.0, 1.0, 0.0],
    },
    VertexRawWithTangents {
        position: [-0.5, 0.0, 0.5],
        tex_coord: [0.0, 1.0],
        normal: [0.0, 1.0, 0.0],
        tangent: [0.0, 1.0, 0.0],
        bitangent: [0.0, 1.0, 0.0],
    },
    VertexRawWithTangents {
        position: [0.5, 0.0, -0.5],
        tex_coord: [1.0, 0.0],
        normal: [0.0, 1.0, 0.0],
        tangent: [0.0, 1.0, 0.0],
        bitangent: [0.0, 1.0, 0.0],
    },
    VertexRawWithTangents {
        position: [0.5, 0.0, 0.5],
        tex_coord: [1.0, 1.0],
        normal: [0.0, 1.0, 0.0],
        tangent: [0.0, 1.0, 0.0],
        bitangent: [0.0, 1.0, 0.0],
    },
];

const SQUARE_INDICES: &'static [u32] = &[3, 2, 1, 2, 0, 1];

pub fn square(render_device: &wgpu::Device) -> Primitive {
    let vertex_buffer = render_device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Square Vertex Buffer"),
        contents: bytemuck::cast_slice(&SQUARE_VERTICES),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let index_buffer = render_device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Square Index Buffer"),
        contents: bytemuck::cast_slice(&SQUARE_INDICES),
        usage: wgpu::BufferUsages::INDEX,
    });

    Primitive {
        index_data: BufferWithLength {
            buffer: index_buffer,
            count: SQUARE_INDICES.len() as u32,
        },
        vertex_buffer,
    }
}
