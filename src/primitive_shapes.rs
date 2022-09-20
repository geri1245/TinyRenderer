use wgpu::util::DeviceExt;

use crate::vertex::VertexRaw;

pub struct PrimitiveShape {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
}

impl PrimitiveShape {
    const SQUARE_VERTICES: &'static [VertexRaw] = &[
        VertexRaw {
            position: [-0.5, 0.0, -0.5],
            tex_coord: [0.0, 0.0],
            normal: [0.0, 1.0, 0.0],
        },
        VertexRaw {
            position: [-0.5, 0.0, 0.5],
            tex_coord: [0.0, 1.0],
            normal: [0.0, 1.0, 0.0],
        },
        VertexRaw {
            position: [0.5, 0.0, -0.5],
            tex_coord: [1.0, 0.0],
            normal: [0.0, 1.0, 0.0],
        },
        VertexRaw {
            position: [0.5, 0.0, 0.5],
            tex_coord: [1.0, 1.0],
            normal: [0.0, 1.0, 0.0],
        },
    ];

    const SQUARE_INDICES: &'static [u16] = &[3, 2, 1, 2, 0, 1];

    pub fn square(render_device: &wgpu::Device) -> PrimitiveShape {
        let vertex_buffer = render_device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Square Vertex Buffer"),
            contents: bytemuck::cast_slice(&Self::SQUARE_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = render_device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Square Index Buffer"),
            contents: bytemuck::cast_slice(&Self::SQUARE_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            vertex_buffer,
            index_buffer,
        }
    }
}
