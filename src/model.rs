use std::{collections::HashMap, path::PathBuf, rc::Rc};

use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};
use wgpu::{util::DeviceExt, Device, Queue, RenderPass};

use crate::{
    components::TransformComponent,
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
    Debug,
    Copy,
    Clone,
    bytemuck::Pod,
    bytemuck::Zeroable,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    ui_item_derive::UiDisplayable,
    ui_item_derive::UiSettableNew,
)]
pub struct PbrParameters {
    pub albedo: Vec3,
    pub roughness: f32,
    pub metalness: f32,

    #[serde(skip_serializing)]
    #[serde(default)]
    #[ui_set(skip)]
    #[ui_param(skip)]
    _padding: [u32; 3],
}

impl Default for PbrParameters {
    fn default() -> Self {
        Self {
            albedo: [1.0, 0.0, 0.0].into(),
            roughness: 1.0,
            metalness: 0.0,
            _padding: [0, 0, 0],
        }
    }
}

impl PbrParameters {
    pub fn new(albedo: Vec3, roughness: f32, metalness: f32) -> Self {
        Self {
            albedo: albedo.into(),
            roughness,
            metalness,
            _padding: [0, 0, 0],
        }
    }
}

#[derive(Default, Debug, Clone, Copy, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum DirtyState {
    /// No changes, nothing needs to be updated
    #[default]
    NothingChanged,
    /// In this case we might have to regenerate the buffers, as the number of items might have changed
    TransformChanged,
    /// In this case it's enough to copy the new data to the existing buffers,
    /// as the number/structure of items remains the same
    EverythingChanged,
}

#[derive(
    Default,
    Debug,
    Clone,
    Copy,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    PartialOrd,
    ui_item_derive::UiDisplayable,
    ui_item_derive::UiSettableNew,
)]
pub enum RenderingPass {
    #[default]
    DeferredMain,
    ForceForwardAfterDeferred,
}

pub fn default_true() -> bool {
    true
}

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Eq,
    PartialOrd,
    Hash,
    ui_item_derive::UiDisplayable,
    ui_item_derive::UiSettableNew,
)]
pub enum PbrRenderingType {
    #[default]
    Textures,
    FlatParameters,
}

#[derive(
    Debug,
    Clone,
    Copy,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    PartialOrd,
    ui_item_derive::UiDisplayable,
    ui_item_derive::UiSettableNew,
)]
pub struct ModelRenderingOptions {
    pub pass: RenderingPass,
    /// Should we use depth testing when rendering this object? If not, then it will be drawn to the screen even if
    /// this object is behind something else
    #[serde(default = "default_true")]
    pub use_depth_test: bool,
    /// Should this object cast shadows? If not, it won't be rendered into the shadow map
    #[serde(default = "default_true")]
    pub cast_shadows: bool,

    pub pbr_resource_type: PbrRenderingType,
}

impl Default for ModelRenderingOptions {
    fn default() -> Self {
        Self {
            cast_shadows: true,
            use_depth_test: true,
            pbr_resource_type: PbrRenderingType::default(),

            pass: Default::default(),
        }
    }
}

#[derive(
    Default,
    Debug,
    Clone,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    ui_item_derive::UiDisplayable,
    ui_item_derive::UiSettableNew,
)]
pub struct ModelDescriptor {
    pub mesh_descriptor: MeshDescriptor,
    pub material_descriptor: PbrMaterialDescriptor,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RenderableDescription {
    pub model_descriptor: ModelDescriptor,
    pub rendering_options: ModelRenderingOptions,
    pub transform: TransformComponent,
}

/// A part of a renderable object. If a renderable consists of multiple parts, then each part is described
/// by this struct. Simple objects have only a single part
#[derive(Debug)]
pub struct RenderablePart {
    /// The topology of the renderable, containing vertex and index data
    pub primitive: Rc<Primitive>,
    /// The material data, in the form of a bind group
    pub material_render_data: MaterialRenderData,
    /// Transformation relative to the parent renderable
    pub local_transform: TransformComponent,
}

#[derive(Debug)]
pub struct Renderable {
    pub id: u32,
    pub description: RenderableDescription,

    pub renderable_parts: Vec<RenderablePart>,
    // Contains the data about the instances. The number of them and the transformation of each instance
    // Currently no instancing is used, so this will always contain a single transform
    pub instance_data: BufferWithLength,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
    ui_item_derive::UiDisplayable,
    ui_item_derive::UiSettableNew,
)]
pub enum MeshDescriptor {
    PrimitiveInCode(PrimitiveShape),
    FromFile(PathBuf),
}

impl Default for MeshDescriptor {
    fn default() -> Self {
        Self::PrimitiveInCode(PrimitiveShape::default())
    }
}

#[derive(Debug)]
pub struct BufferWithLength {
    pub buffer: wgpu::Buffer,
    pub count: u32,
}

impl Renderable {
    pub fn new(
        renderable_description: RenderableDescription,
        renderable_parts: Vec<RenderablePart>,
        device: &wgpu::Device,
        object_id: u32,
    ) -> Self {
        let instance_data =
            create_instance_buffer(&renderable_description.transform, object_id, device);

        Self {
            id: object_id,
            description: renderable_description,
            renderable_parts,
            instance_data,
        }
    }

    pub fn render<'a>(
        &'a self,
        render_pass: &mut RenderPass<'a>,
        material_group_index: Option<u32>,
    ) {
        for part in &self.renderable_parts {
            if let Some(material_group_index) = material_group_index {
                part.material_render_data
                    .bind_render_pass(render_pass, material_group_index);
            }

            render_pass.set_vertex_buffer(0, part.primitive.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.instance_data.buffer.slice(..));
            render_pass.set_index_buffer(
                part.primitive.index_data.buffer.slice(..),
                wgpu::IndexFormat::Uint32,
            );
            render_pass.draw_indexed(
                0..part.primitive.index_data.count,
                0,
                0..self.instance_data.count,
            );
        }
    }

    pub fn update_transform_render_state(
        &mut self,
        queue: &Queue,
        new_transform: &TransformComponent,
        object_id: u32,
    ) {
        queue.write_buffer(
            &self.instance_data.buffer,
            0,
            bytemuck::cast_slice(&[new_transform.to_raw(object_id)]),
        );
        self.instance_data.count = 1;
    }

    pub fn update_material_render_state(
        &mut self,
        device: &Device,
        new_material: &PbrMaterialDescriptor,
    ) {
        // TODO: this should only update a specific material, not all of them.
        // As a first solution, it would be enough to just recreate the entire renderable state when something changes
        // This is very much not optimal, but it would be an easy first version to implement
        for part in &mut self.renderable_parts {
            match new_material {
                PbrMaterialDescriptor::Texture(_vec) => {
                    // todo!()
                }
                PbrMaterialDescriptor::Flat(pbr_parameters) => {
                    part.material_render_data =
                        MaterialRenderData::from_flat_parameters(device, pbr_parameters);
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct Primitive {
    pub vertex_buffer: wgpu::Buffer,
    pub index_data: BufferWithLength,
}

impl Primitive {
    pub fn render<'a>(&'a self, render_pass: &mut RenderPass<'a>) {
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_data.buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..self.index_data.count, 0, 0..1);
    }
}

#[derive(Debug, serde::Serialize)]
pub struct InstanceData {
    pub instances: Vec<TransformComponent>,
}

pub fn create_instance_buffer(
    transform: &TransformComponent,
    object_id: u32,
    device: &Device,
) -> BufferWithLength {
    let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Square Instance Buffer"),
        contents: bytemuck::cast_slice(&[transform.to_raw(object_id)]),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    });

    BufferWithLength {
        buffer: instance_buffer,
        count: 1,
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
        path: PathBuf,
        positions: &[Vec3],
        normals: &[Vec3],
        tex_coords: &[Vec2],
        indices: &[u32],
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
