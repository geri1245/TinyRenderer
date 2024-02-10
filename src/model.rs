use std::rc::Rc;

use serde::Deserialize;

use crate::texture;

#[derive(Deserialize)]
pub struct ModelDescriptorTextureDescription {
    #[serde(default)]
    pub albedo: String,
    #[serde(default)]
    pub roughness: String,
    #[serde(default)]
    pub metalness: String,
    #[serde(default)]
    pub normal: String,
}

#[derive(Deserialize)]
pub struct ModelDescriptorFile {
    pub model: String,
    #[serde(default)]
    pub textures: Vec<ModelDescriptorTextureDescription>,
}

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
