use std::rc::Rc;

use wgpu::util::DeviceExt;

use crate::model::{Material, Mesh};
use crate::vertex::VertexRaw;

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

const SQUARE_INDICES: &'static [u32] = &[3, 2, 1, 2, 0, 1];

pub fn square(render_device: &wgpu::Device, material: Option<Rc<Material>>) -> Mesh {
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

    Mesh {
        name: "Primitive_Square".into(),
        index_buffer,
        index_count: SQUARE_INDICES.len() as u32,
        vertex_buffer,
        material,
    }
}
