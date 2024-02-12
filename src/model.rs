use std::{collections::HashMap, rc::Rc};

use glam::{Vec2, Vec3};
use serde::Deserialize;
use wgpu::{util::DeviceExt, Device};

use crate::{texture, vertex::VertexRawWithTangents};

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

impl Mesh {
    pub fn new(
        device: &Device,
        name: String,
        positions: Vec<f32>,
        normals: Vec<f32>,
        tex_coords: Vec<f32>,
        indices: Vec<u32>,
        material: Rc<Material>,
    ) -> Self {
        let mut vertices = (0..positions.len() / 3)
            .map(|i| VertexRawWithTangents {
                position: [positions[i * 3], positions[i * 3 + 1], positions[i * 3 + 2]],
                tex_coord: [tex_coords[i * 2], tex_coords[i * 2 + 1]],
                normal: [normals[i * 3], normals[i * 3 + 1], normals[i * 3 + 2]],
                tangent: [0.0; 3],
                bitangent: [0.0; 3],
            })
            .collect::<Vec<_>>();

        let mut triangles_included = vec![0u32; vertices.len()];

        // Calculate tangents and bitangets. We're going to
        // use the triangles, so we need to loop through the
        // indices in chunks of 3
        // Method taken from https://learnopengl.com/Advanced-Lighting/Normal-Mapping - Tangent space
        for c in indices.chunks(3) {
            let v0 = &vertices[c[0] as usize];
            let v1 = &vertices[c[1] as usize];
            let v2 = &vertices[c[2] as usize];

            let pos0: Vec3 = v0.position.into();
            let pos1: Vec3 = v1.position.into();
            let pos2: Vec3 = v2.position.into();

            let uv0: Vec2 = v0.tex_coord.into();
            let uv1: Vec2 = v1.tex_coord.into();
            let uv2: Vec2 = v2.tex_coord.into();

            // Calculate the edges of the triangle
            let edge1 = pos1 - pos0;
            let edge2 = pos2 - pos0;

            // Calculate the UV space difference of the vectors
            let delta_uv1 = uv1 - uv0;
            let delta_uv2 = uv2 - uv0;

            // Solving the following system of equations will
            // give us the tangent and bitangent.
            //     edge1 = delta_uv1.x * T + delta_u.y * B
            //     edge2 = delta_uv2.x * T + delta_uv2.y * B
            // We basically want to express the edges with a new Tangent and Bitangent
            // vector that is in the same space as our uv coordinates
            let r = 1.0 / (delta_uv1.x * delta_uv2.y - delta_uv2.x * delta_uv1.y);
            let tangent = (edge1 * delta_uv2.y - edge2 * delta_uv1.y) * r;
            let bitangent = (edge2 * delta_uv1.x - edge1 * delta_uv2.x) * -r;

            // Some vertices are part of multiple faces, so we just sum them here
            // and we will average them in a next pass.
            vertices[c[0] as usize].tangent =
                (tangent + Vec3::from(vertices[c[0] as usize].tangent)).into();
            vertices[c[1] as usize].tangent =
                (tangent + Vec3::from(vertices[c[1] as usize].tangent)).into();
            vertices[c[2] as usize].tangent =
                (tangent + Vec3::from(vertices[c[2] as usize].tangent)).into();
            vertices[c[0] as usize].bitangent =
                (bitangent + Vec3::from(vertices[c[0] as usize].bitangent)).into();
            vertices[c[1] as usize].bitangent =
                (bitangent + Vec3::from(vertices[c[1] as usize].bitangent)).into();
            vertices[c[2] as usize].bitangent =
                (bitangent + Vec3::from(vertices[c[2] as usize].bitangent)).into();

            triangles_included[c[0] as usize] += 1;
            triangles_included[c[1] as usize] += 1;
            triangles_included[c[2] as usize] += 1;
        }

        // Average the tangents/bitangents
        for (i, n) in triangles_included.into_iter().enumerate() {
            let denom = 1.0 / n as f32;
            let v = &mut vertices[i];
            v.tangent = (Vec3::from(v.tangent) * denom).into();
            v.bitangent = (Vec3::from(v.bitangent) * denom).into();
        }

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{:?} Vertex Buffer", name)),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{:?} Index Buffer", name)),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Mesh {
            name: name,
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
            material,
        }
    }
}
