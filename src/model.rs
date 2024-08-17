use std::{collections::HashMap, rc::Rc};

use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use wgpu::{util::DeviceExt, Device, RenderPass};

use crate::{
    instance::SceneComponent,
    material::{MaterialRenderData, PbrMaterialDescriptor},
    resource_loader::PrimitiveShape,
    texture::TextureUsage,
    vertex::VertexRawWithTangents,
};

#[derive(Serialize, Deserialize)]
pub struct ModelDescriptorFile {
    pub model: String,
    #[serde(default)]
    pub textures: HashMap<TextureUsage, String>,
}

#[repr(C)]
#[derive(
    Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, serde::Serialize, serde::Deserialize,
)]
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ObjectWithMaterial {
    pub mesh_source: MeshSource,
    pub material_descriptor: PbrMaterialDescriptor,
}

#[derive(Debug, serde::Serialize)]
pub struct Renderable {
    pub mesh_descriptor: ObjectWithMaterial,
    pub instances: Vec<SceneComponent>,

    #[serde(skip)]
    pub instance_render_data: BufferWithLength,
    #[serde(skip)]
    pub vertex_render_data: Rc<Primitive>,
    #[serde(skip)]
    pub material_render_data: MaterialRenderData,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum MeshSource {
    PrimitiveInCode(PrimitiveShape),
    FromFile(String),
}

#[derive(Debug)]
pub struct BufferWithLength {
    pub buffer: wgpu::Buffer,
    pub count: u32,
}

#[derive(Debug)]
pub struct Primitive {
    pub vertex_buffer: wgpu::Buffer,
    pub index_data: BufferWithLength,
}

#[derive(Debug, serde::Serialize)]
pub struct InstanceData {
    pub instances: Vec<SceneComponent>,
}

impl Renderable {
    pub fn new(
        mesh_descriptor: ObjectWithMaterial,
        instances: Vec<SceneComponent>,
        primitive: Rc<Primitive>,
        material_render_data: MaterialRenderData,
        device: &wgpu::Device,
    ) -> Self {
        let instance_data = create_instance_buffer(&instances, device);

        Self {
            mesh_descriptor,
            instances,
            vertex_render_data: primitive,
            instance_render_data: instance_data,
            material_render_data,
        }
    }

    pub fn render<'a>(&'a self, render_pass: &mut RenderPass<'a>, use_material: bool) {
        if use_material {
            self.material_render_data.bind_render_pass(render_pass, 0);
        }

        render_pass.set_vertex_buffer(0, self.vertex_render_data.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.instance_render_data.buffer.slice(..));
        render_pass.set_index_buffer(
            self.vertex_render_data.index_data.buffer.slice(..),
            wgpu::IndexFormat::Uint32,
        );
        render_pass.draw_indexed(
            0..self.vertex_render_data.index_data.count,
            0,
            0..self.instance_render_data.count,
        );
    }
}

pub fn create_instance_buffer(
    instances: &Vec<SceneComponent>,
    device: &Device,
) -> BufferWithLength {
    let raw_instances = instances
        .iter()
        .map(|instance| instance.to_raw())
        .collect::<Vec<_>>();
    let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Square Instance Buffer"),
        contents: bytemuck::cast_slice(&raw_instances),
        usage: wgpu::BufferUsages::VERTEX,
    });

    BufferWithLength {
        buffer: instance_buffer,
        count: instances.len() as u32,
    }
}

impl Primitive {
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
        path: String,
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
            label: Some(&format!("{:?} Vertex Buffer", path)),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{:?} Index Buffer", path)),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            index_data: BufferWithLength {
                buffer: index_buffer,
                count: indices.len() as u32,
            },
            vertex_buffer,
        }
    }
}
