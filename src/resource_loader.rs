use std::collections::HashMap;
use std::fs::File;
use std::rc::Rc;

use anyhow::anyhow;
use gltf::Gltf;
use std::io::BufReader;
use std::path::PathBuf;
use tobj::MTLLoadResult;
use wgpu::{CommandEncoderDescriptor, Device, Extent3d, Queue};

use glam::{Vec2, Vec3};

use crate::mipmap_generator::MipMapGenerator;
use crate::model::ObjectWithMaterial;
use crate::primitive_shapes::square;
use crate::texture::{SamplingType, TextureSourceDescriptor};
use crate::{
    file_loader::ImageLoader,
    material::{MaterialRenderData, PbrMaterialDescriptor},
    model::{MeshSource, Primitive},
    texture::{SampledTexture, TextureUsage},
};

pub struct LoadedModelWithMaterial {
    pub primitive: Rc<Primitive>,
    pub material: MaterialRenderData,
}

#[derive(
    Debug, Hash, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, PartialOrd, Ord,
)]
pub enum PrimitiveShape {
    Cube,
    Square,
}

pub struct ResourceLoader {
    pub default_mat: Rc<MaterialRenderData>,
    default_textures: HashMap<TextureUsage, Rc<SampledTexture>>,
    primitive_shapes: HashMap<PrimitiveShape, Rc<Primitive>>,
}

impl ResourceLoader {
    pub async fn new(device: &Device, queue: &Queue) -> Self {
        let (default_mat, default_textures) = Self::load_default_textures(device, queue);
        let primitive_shapes = Self::load_primitive_shapes(device).unwrap();

        let loader = ResourceLoader {
            default_mat,
            default_textures,
            primitive_shapes,
        };

        loader
    }

    pub fn get_default_material(&self) -> Rc<MaterialRenderData> {
        self.default_mat.clone()
    }

    pub fn get_primitive_shape(&self, shape: PrimitiveShape) -> Rc<Primitive> {
        self.primitive_shapes.get(&shape).unwrap().clone()
    }

    fn load_primitive_shapes(
        device: &Device,
    ) -> anyhow::Result<HashMap<PrimitiveShape, Rc<Primitive>>> {
        let mesh = Rc::new(load_obj(device, "assets/models/cube/cube.obj".into())?);

        let mut primitive_shapes = HashMap::new();
        primitive_shapes.insert(PrimitiveShape::Cube, mesh);
        primitive_shapes.insert(PrimitiveShape::Square, Rc::new(square(device)));

        return Ok(primitive_shapes);
    }

    fn load_default_textures(
        device: &Device,
        queue: &Queue,
    ) -> (
        Rc<MaterialRenderData>,
        HashMap<TextureUsage, Rc<SampledTexture>>,
    ) {
        const TEXTURES: [(&[u8], &'static str, TextureUsage); 4] = [
            (
                include_bytes!("../assets/textures/defaults/albedo.png"),
                "assets/textures/defaults/albedo.png",
                TextureUsage::Albedo,
            ),
            (
                include_bytes!("../assets/textures/defaults/normal.png"),
                "assets/textures/defaults/normal.png",
                TextureUsage::Normal,
            ),
            (
                include_bytes!("../assets/textures/defaults/metalness.png"),
                "assets/textures/defaults/metalness.png",
                TextureUsage::Metalness,
            ),
            (
                include_bytes!("../assets/textures/defaults/roughness.png"),
                "assets/textures/defaults/roughness.png",
                TextureUsage::Roughness,
            ),
        ];

        let mut default_material_textures = HashMap::new();

        for (data, path, usage) in TEXTURES {
            let texture = Rc::new(
                SampledTexture::from_image_bytes(device, queue, data, usage, Some(path)).unwrap(),
            );
            default_material_textures.insert(usage, texture);
        }

        (
            Rc::new(MaterialRenderData::new(device, &default_material_textures)),
            default_material_textures,
        )
    }

    fn load_texture(
        &self,
        descriptor: &TextureSourceDescriptor,
        device: &Device,
        queue: &Queue,
    ) -> anyhow::Result<Rc<SampledTexture>> {
        match &descriptor.source {
            crate::texture::MaterialSource::FromFile(path) => {
                let image = ImageLoader::try_load_image(async_std::path::PathBuf::from(path))?;
                let texture_size = Extent3d {
                    width: image.width(),
                    height: image.height(),
                    depth_or_array_layers: 1,
                };
                Ok(Rc::new(
                    SampledTexture::from_image(
                        &device,
                        queue,
                        &image,
                        texture_size,
                        descriptor.usage,
                        SamplingType::Linear,
                        Some(&path),
                    )
                    .unwrap(),
                ))
            }
            crate::texture::MaterialSource::Defaults(usage) => Ok(self
                .default_textures
                .get(usage)
                .ok_or(anyhow!("Could not find default texture for {usage:?}"))?
                .clone()),
        }
    }

    pub fn load_model(
        &self,
        mesh_descriptor: &ObjectWithMaterial,
        device: &Device,
        queue: &Queue,
        mip_map_generator: &MipMapGenerator,
    ) -> anyhow::Result<LoadedModelWithMaterial> {
        let primitive = match &mesh_descriptor.mesh_source {
            MeshSource::PrimitiveInCode(shape) => self.primitive_shapes.get(shape).unwrap().clone(),
            MeshSource::FromFile(path) => {
                if let Some(extension) = path.extension() {
                    if extension == "obj" {
                        Rc::new(load_obj(&device, path.clone())?)
                    } else if extension == "gltf" {
                        Rc::new(load_gltf(device, path.clone())?)
                    } else {
                        return Err(anyhow!(
                            "Resource loading not yet implemented for file type {extension:?}"
                        ));
                    }
                } else {
                    return Err(anyhow!("Failed to get extension of file {path:?}"));
                }
            }
        };

        let material = match &mesh_descriptor.material_descriptor {
            PbrMaterialDescriptor::Texture(textures) => {
                let mut loaded_textures = HashMap::with_capacity(textures.len());
                for texture_descriptor in textures {
                    let texture = self.load_texture(texture_descriptor, device, queue)?;
                    match texture_descriptor.usage {
                        TextureUsage::Albedo | TextureUsage::Normal => {
                            let mut encoder =
                                device.create_command_encoder(&CommandEncoderDescriptor {
                                    label: Some("mipmap generator encoder"),
                                });
                            mip_map_generator.create_mips_for_texture(
                                &mut encoder,
                                &(*texture),
                                None,
                                device,
                            );
                            queue.submit(Some(encoder.finish()));
                        }
                        TextureUsage::Metalness
                        | TextureUsage::Roughness
                        | TextureUsage::HdrAlbedo => {}
                    }
                    loaded_textures.insert(texture_descriptor.usage, texture);
                }
                for (usage, texture) in &self.default_textures {
                    if !loaded_textures.contains_key(&usage) {
                        loaded_textures.insert(*usage, texture.clone());
                    }
                }
                MaterialRenderData::new(device, &loaded_textures)
            }
            PbrMaterialDescriptor::Flat(pbr_parameters) => {
                MaterialRenderData::from_flat_parameters(device, pbr_parameters)
            }
        };

        Ok(LoadedModelWithMaterial {
            primitive,
            material,
        })
    }
}

fn vec_to_vec3s(values: Vec<f32>) -> Vec<Vec3> {
    values
        .chunks(3)
        .map(|vec| Vec3::new(vec[0], vec[1], vec[2]))
        .collect()
}

fn vec_to_vec2s(values: Vec<f32>) -> Vec<Vec2> {
    values
        .chunks(2)
        .map(|vec| Vec2::new(vec[0], vec[1]))
        .collect()
}

pub fn load_gltf(device: &wgpu::Device, asset_path: PathBuf) -> anyhow::Result<Primitive> {
    let gltf = Gltf::open(asset_path)?;
    for scene in gltf.scenes() {
        for node in scene.nodes() {
            println!(
                "Node #{} has {} children",
                node.index(),
                node.children().count(),
            );
        }
    }

    Err(anyhow!("alma"))

    // let mut positions = Vec::new();
    // let mut normals = Vec::new();
    // let mut tex_coords = Vec::new();
    // let mut indices = Vec::new();

    // // Each model loaded by tobj is a self-standing model, meaning that it will contain all the positions/normals
    // // etc. that it needs, unlike in the obj format, where each model can reference prebious positions, etc. that
    // // do not strictly belongs to them. Thus when combining the models into a single model, we need to increase the
    // // index values by the number of position parameters that were before this one. We divide by 3, because
    // // at this point the Vec3s are flattened out, but we will use the indices to index a Vec<Vec3>
    // let mut index_offset = 0;

    // for model in models {
    //     positions.extend(&model.mesh.positions);
    //     normals.extend(&model.mesh.normals);
    //     tex_coords.extend(&model.mesh.texcoords);
    //     indices.extend(model.mesh.indices.iter().map(|index| index + index_offset));

    //     index_offset += (model.mesh.positions.len() / 3) as u32;
    // }

    // Ok(Primitive::new(
    //     device,
    //     asset_path,
    //     &vec_to_vec3s(positions),
    //     &vec_to_vec3s(normals),
    //     &vec_to_vec2s(tex_coords),
    //     &indices,
    // ))
}

pub fn load_obj(device: &wgpu::Device, asset_path: PathBuf) -> anyhow::Result<Primitive> {
    let mut file_reader = BufReader::new(File::open(&asset_path)?);
    let (models, _obj_materials) =
        tobj::load_obj_buf(&mut file_reader, &tobj::GPU_LOAD_OPTIONS, |_| {
            // We don't care about the mtl file, so this is just a dummy loader implementation
            MTLLoadResult::Ok((Default::default(), Default::default()))
        })?;

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut tex_coords = Vec::new();
    let mut indices = Vec::new();

    // Each model loaded by tobj is a self-standing model, meaning that it will contain all the positions/normals
    // etc. that it needs, unlike in the obj format, where each model can reference prebious positions, etc. that
    // do not strictly belongs to them. Thus when combining the models into a single model, we need to increase the
    // index values by the number of position parameters that were before this one. We divide by 3, because
    // at this point the Vec3s are flattened out, but we will use the indices to index a Vec<Vec3>
    let mut index_offset = 0;

    for model in models {
        positions.extend(&model.mesh.positions);
        normals.extend(&model.mesh.normals);
        tex_coords.extend(&model.mesh.texcoords);
        indices.extend(model.mesh.indices.iter().map(|index| index + index_offset));

        index_offset += (model.mesh.positions.len() / 3) as u32;
    }

    Ok(Primitive::new(
        device,
        asset_path,
        &vec_to_vec3s(positions),
        &vec_to_vec3s(normals),
        &vec_to_vec2s(tex_coords),
        &indices,
    ))
}
