use wgpu::util::DeviceExt;

use crate::drawable::Drawable;
use crate::vertex::VertexRaw;

pub struct PrimitiveShape {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
}

pub struct TexturedPrimitive {
    pub primitive_shape: PrimitiveShape,
    pub texture_bind_group: wgpu::BindGroup,
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

    const SQUARE_INDICES: &'static [u32] = &[3, 2, 1, 2, 0, 1];

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
            index_count: Self::SQUARE_INDICES.len() as u32,
        }
    }
}

impl<'a, 'b> Drawable<'a, 'b> for PrimitiveShape
where
    'a: 'b,
{
    fn draw_instanced(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'b>,
        instances: std::ops::Range<u32>,
    ) {
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.draw_indexed(0..self.index_count, 0, instances);
    }
}

impl<'a, 'b> Drawable<'a, 'b> for TexturedPrimitive
where
    'a: 'b,
{
    fn draw_instanced(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'b>,
        instances: std::ops::Range<u32>,
    ) {
        render_pass.set_bind_group(2, &self.texture_bind_group, &[]);
        self.primitive_shape.draw_instanced(render_pass, instances);
    }
}
