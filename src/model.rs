use std::{collections::HashMap, rc::Rc};

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
}

#[derive(PartialEq, Eq, Hash, Clone)]
pub enum TextureType {
    Rough,
    Metal,
    Albedo,
    Normal,
}

pub struct TextureData {
    pub name: String,
    pub texture: texture::SampledTexture,
    pub bind_group: wgpu::BindGroup,
    pub texture_type: TextureType,
}

pub struct Material {
    textures: HashMap<TextureType, TextureData>,
}

impl Material {
    pub fn new() -> Self {
        Material {
            textures: HashMap::new(),
        }
    }

    pub fn add_texture(&mut self, texture_type: TextureType, texture_data: TextureData) {
        self.textures.insert(texture_type.clone(), texture_data);
    }

    pub fn get(&self, texture_type: &TextureType) -> Option<&TextureData> {
        self.textures.get(texture_type)
    }
}

pub struct Mesh {
    pub name: String,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
    pub material: Rc<Material>,
}
