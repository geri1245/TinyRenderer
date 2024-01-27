use std::rc::Rc;

use crate::basic_renderable::BasicRenderable;
use crate::texture;

pub struct Model {
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Rc<Material>>,
}

pub struct Material {
    pub name: String,
    pub diffuse_texture: texture::Texture,
    pub bind_group: wgpu::BindGroup,
}

pub struct Mesh {
    pub name: String,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
    pub material: Option<Rc<Material>>,
}

impl BasicRenderable for Mesh {
    // pub fn render(&self, renderer: &Renderer) {}
    fn get_vertex_buffer(&self) -> &wgpu::Buffer {
        &self.vertex_buffer
    }

    fn get_index_buffer(&self) -> &wgpu::Buffer {
        &self.index_buffer
    }

    fn get_index_count(&self) -> u32 {
        self.index_count
    }
}
