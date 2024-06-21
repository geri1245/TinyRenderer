use std::{collections::HashMap, rc::Rc};

use async_std::path::PathBuf;
use glam::{Vec2, Vec3};
use serde::Deserialize;
use wgpu::{util::DeviceExt, Device, RenderPass};

use crate::{
    instance::SceneComponent, material::Material, texture::TextureUsage,
    vertex::VertexRawWithTangents,
};

#[derive(Deserialize)]
pub struct ModelDescriptorFile {
    pub model: String,
    #[serde(default)]
    pub textures: HashMap<TextureUsage, String>,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PbrParameters {
    pub albedo: [f32; 3],
    pub roughness: f32,
    pub metalness: f32,
    _padding: [u32; 3],
}

impl Default for PbrParameters {
    fn default() -> Self {
        Self {
            albedo: [1.0, 0.0, 0.0],
            roughness: 1.0,
            metalness: 0.0,
            _padding: [0, 0, 0],
        }
    }
}

impl PbrParameters {
    pub fn fully_rough(albedo: [f32; 3]) -> Self {
        Self {
            albedo,
            ..Default::default()
        }
    }

    pub fn new(albedo: [f32; 3], roughness: f32, metalness: f32) -> Self {
        Self {
            albedo,
            roughness,
            metalness,
            _padding: [0, 0, 0],
        }
    }
}

pub struct ModelLoadingData {
    pub path: PathBuf,
    pub textures: Vec<(TextureUsage, PathBuf)>,
}

pub struct RenderableMesh {
    pub name: String,
    pub path: Option<String>,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
}

pub struct InstanceData {
    pub instances: Vec<SceneComponent>,
    pub instance_buffer: wgpu::Buffer,
}

#[derive()]
pub struct RenderableObject {
    pub mesh: Rc<RenderableMesh>,
    pub material: Rc<Material>,
    pub material_id: Option<u32>,
    pub instance_data: InstanceData,
}

impl RenderableObject {
    pub fn render<'a>(&'a self, render_pass: &mut RenderPass<'a>, use_material: bool) {
        if use_material {
            self.material.bind_render_pass(render_pass, 0);
        }

        render_pass.set_vertex_buffer(0, self.mesh.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.instance_data.instance_buffer.slice(..));
        render_pass.set_index_buffer(self.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(
            0..self.mesh.index_count,
            0,
            0..self.instance_data.instances.len() as u32,
        );
    }
}

impl InstanceData {
    pub fn new(instances: Vec<SceneComponent>, device: &Device) -> Self {
        let raw_instances = instances
            .iter()
            .map(|instance| instance.to_raw())
            .collect::<Vec<_>>();
        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Square Instance Buffer"),
            contents: bytemuck::cast_slice(&raw_instances),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Self {
            instance_buffer,
            instances,
        }
    }
}

impl RenderableMesh {
    // fn calculate_tangetns_bitangents(indices: Vec3,) -> (Vec3, Vec3) {
    //     for c in indices.chunks(3) {
    //         let v0 = &vertices[c[0] as usize];
    //         let v1 = &vertices[c[1] as usize];
    //         let v2 = &vertices[c[2] as usize];

    //         let pos0: Vec3 = v0.position.into();
    //         let pos1: Vec3 = v1.position.into();
    //         let pos2: Vec3 = v2.position.into();

    //         let uv0: Vec2 = v0.tex_coord.into();
    //         let uv1: Vec2 = v1.tex_coord.into();
    //         let uv2: Vec2 = v2.tex_coord.into();

    //         // Calculate the edges of the triangle
    //         let edge1 = pos1 - pos0;
    //         let edge2 = pos2 - pos0;

    //         // Calculate the UV space difference of the vectors
    //         let delta_uv1 = uv1 - uv0;
    //         let delta_uv2 = uv2 - uv0;

    //         // Solving the following system of equations will
    //         // give us the tangent and bitangent.
    //         //     edge1 = delta_uv1.x * T + delta_u.y * B
    //         //     edge2 = delta_uv2.x * T + delta_uv2.y * B
    //         // We basically want to express the edges with a new Tangent and Bitangent
    //         // vector that is in the same space as our uv coordinates
    //         let r = 1.0 / (delta_uv1.x * delta_uv2.y - delta_uv2.x * delta_uv1.y);
    //         let tangent = (edge1 * delta_uv2.y - edge2 * delta_uv1.y) * r;
    //         let bitangent = (edge2 * delta_uv1.x - edge1 * delta_uv2.x) * -r;

    //         // Some vertices are part of multiple faces, so we just sum them here
    //         // and we will average them in a next pass.
    //         vertices[c[0] as usize].tangent =
    //             (tangent + Vec3::from(vertices[c[0] as usize].tangent)).into();
    //         vertices[c[1] as usize].tangent =
    //             (tangent + Vec3::from(vertices[c[1] as usize].tangent)).into();
    //         vertices[c[2] as usize].tangent =
    //             (tangent + Vec3::from(vertices[c[2] as usize].tangent)).into();
    //         vertices[c[0] as usize].bitangent =
    //             (bitangent + Vec3::from(vertices[c[0] as usize].bitangent)).into();
    //         vertices[c[1] as usize].bitangent =
    //             (bitangent + Vec3::from(vertices[c[1] as usize].bitangent)).into();
    //         vertices[c[2] as usize].bitangent =
    //             (bitangent + Vec3::from(vertices[c[2] as usize].bitangent)).into();

    //         triangles_included[c[0] as usize] += 1;
    //         triangles_included[c[1] as usize] += 1;
    //         triangles_included[c[2] as usize] += 1;
    //     }
    // }

    pub fn new(
        device: &Device,
        name: String,
        path: Option<String>,
        positions: Vec<Vec3>,
        normals: Vec<Vec3>,
        tex_coords: Vec<Vec2>,
        indices: Vec<u32>,
    ) -> Self {
        let mut vertices = (0..positions.len())
            .map(|i| VertexRawWithTangents {
                position: positions[i].into(),
                tex_coord: tex_coords[i].into(),
                normal: normals[i].into(),
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

        RenderableMesh {
            name,
            path,
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
        }
    }

    // pub fn from_vertex_raw(raw_vertices: &[VertexRaw]) -> Self {}
}
