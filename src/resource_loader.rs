use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::File;
use std::rc::Rc;

use async_std::{fs, task::block_on};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use tobj::MTLLoadResult;
use wgpu::{Device, Extent3d};

use glam::{Vec2, Vec3};

use crate::instance::SceneComponent;
use crate::model::{ObjectWithMaterial, Renderable};
use crate::texture::{MaterialSource, TextureSourceDescriptor};
use crate::{
    file_loader::FileLoader,
    material::{MaterialRenderData, PbrMaterialDescriptor},
    model::{MeshSource, ModelDescriptorFile, ModelLoadingData, Primitive},
    texture::{SampledTexture, TextureUsage},
};

const ASSETS_FOLDER_NAME: &str = "assets";
const MODELS_FOLDER_NAME: &str = "models";
const TEXTURES_FOLDER_NAME: &str = "textures";
const ASSET_FILE_NAME: &str = "asset.json";

#[derive(Debug, Hash, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PrimitiveShape {
    Cube,
    Square,
}

struct PendingModel {
    model: Rc<Primitive>,
    material: PendingMaterialData,
    instances: Vec<SceneComponent>,
    mesh_descriptor: ObjectWithMaterial,
}

struct PendingMaterialData {
    textures: HashMap<TextureUsage, Rc<SampledTexture>>,
    missing_texture_load_ids: HashSet<u32>,
}

impl PendingMaterialData {
    fn new(missing_ids: HashSet<u32>) -> Self {
        PendingMaterialData {
            textures: HashMap::new(),
            missing_texture_load_ids: missing_ids,
        }
    }

    fn add_texture(
        &mut self,
        texture_load_id: u32,
        texture_usage: TextureUsage,
        texture: Rc<SampledTexture>,
    ) {
        self.textures.insert(texture_usage, texture);
        self.missing_texture_load_ids.remove(&texture_load_id);
    }

    fn is_ready(&self) -> bool {
        self.missing_texture_load_ids.is_empty()
    }

    fn get_material(
        &mut self,
        device: &Device,
        default_textures: &HashMap<TextureUsage, Rc<SampledTexture>>,
    ) -> MaterialRenderData {
        for (usage, texture) in default_textures {
            if !self.textures.contains_key(&usage) {
                self.textures.insert(*usage, texture.clone());
            }
        }

        MaterialRenderData::new(device, &self.textures)
    }
}

pub struct ResourceLoader {
    asset_loader: FileLoader,
    loading_id_to_asset_data: HashMap<u32, TextureSourceDescriptor>,
    pending_models: HashMap<u32, PendingModel>,
    next_material_id: u32,
    texture_id_to_model_id: HashMap<u32, u32>,
    pub default_mat: Rc<MaterialRenderData>,
    default_textures: HashMap<TextureUsage, Rc<SampledTexture>>,
    primitive_shapes: HashMap<PrimitiveShape, Rc<Primitive>>,
}

impl ResourceLoader {
    pub async fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let asset_loader = FileLoader::new();

        let (default_mat, default_textures) = Self::load_default_textures(device, queue);
        let primitive_shapes = Self::load_primitive_shapes(device).await.unwrap();

        let loader = ResourceLoader {
            asset_loader,
            loading_id_to_asset_data: HashMap::new(),
            pending_models: HashMap::new(),
            next_material_id: 0,
            texture_id_to_model_id: HashMap::new(),
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

    async fn load_primitive_shapes(
        device: &Device,
    ) -> anyhow::Result<HashMap<PrimitiveShape, Rc<Primitive>>> {
        let bytes: Vec<u8> = include_bytes!("../assets/models/cube/cube.obj").into();
        let mut reader = BufReader::new(&bytes[..]);
        let mesh = Rc::new(load_obj(&mut reader, device, "cube/cube.obj".into()).await?);

        let mut primitive_shapes = HashMap::new();
        primitive_shapes.insert(PrimitiveShape::Cube, mesh);

        return Ok(primitive_shapes);
    }

    fn load_default_textures(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
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

    fn queue_texture_for_loading(
        &mut self,
        descriptor: &TextureSourceDescriptor,
        id_of_model_being_loaded: u32,
    ) -> Option<u32> {
        let model_being_loaded = self
            .pending_models
            .get_mut(&id_of_model_being_loaded)
            .unwrap();

        match &descriptor.source {
            crate::texture::MaterialSource::FromFile(path) => {
                let loading_id = self
                    .asset_loader
                    .start_loading_bytes(async_std::path::PathBuf::from(path));
                self.loading_id_to_asset_data
                    .insert(loading_id, descriptor.clone());
                model_being_loaded
                    .material
                    .missing_texture_load_ids
                    .insert(loading_id);
                Some(loading_id)
            }
            crate::texture::MaterialSource::Defaults(usage) => {
                let texture = self.default_textures.get(usage).unwrap();
                model_being_loaded
                    .material
                    .textures
                    .insert(*usage, texture.clone());
                None
            }
        }
    }

    // fn queue_material_for_loading(
    //     &mut self,
    //     textures_in_material: Vec<TextureSourceDescriptor>,
    // ) -> u32 {
    //     let current_material_id = self.next_material_id;
    //     self.next_material_id += 1;
    //     let mut pending_texture_ids = HashSet::new();

    //     for texture in textures_in_material {
    //         let loading_id = self.queue_texture_for_loading(texture);
    //         self.texture_id_to_material_id
    //             .insert(loading_id, current_material_id);
    //         pending_texture_ids.insert(loading_id);
    //     }

    //     self.pending_models.insert(
    //         current_material_id,
    //         PendingModel {
    //             model: todo!(),
    //             material: PendingMaterialData::new(pending_texture_ids),
    //         },
    //     );

    //     current_material_id
    // }

    pub fn poll_loaded_textures(
        &mut self,
        device: &Device,
        queue: &wgpu::Queue,
    ) -> Vec<Renderable> {
        let results = self
            .asset_loader
            .poll_loaded_resources()
            .unwrap_or_default();

        let mut renderables_ready = Vec::new();

        for asset_load_result in results {
            // TODO: Error handling
            let pending_texture_data = self
                .loading_id_to_asset_data
                .get(&asset_load_result.id)
                .unwrap();

            if let MaterialSource::FromFile(path) = &pending_texture_data.source {
                let texture_size = Extent3d {
                    width: asset_load_result.loaded_image.width(),
                    height: asset_load_result.loaded_image.height(),
                    depth_or_array_layers: 1,
                };
                let texture = Rc::new(
                    SampledTexture::from_image(
                        &device,
                        queue,
                        &asset_load_result.loaded_image,
                        texture_size,
                        pending_texture_data.usage,
                        Some(&path),
                    )
                    .unwrap(),
                );

                let model_id = self
                    .texture_id_to_model_id
                    .get(&asset_load_result.id)
                    .unwrap();
                let pending_model = self.pending_models.get_mut(&model_id).unwrap();

                pending_model.material.add_texture(
                    asset_load_result.id,
                    pending_texture_data.usage,
                    texture,
                );

                if pending_model.material.is_ready() {
                    let material = pending_model
                        .material
                        .get_material(device, &self.default_textures);
                    let renderable = Renderable::new(
                        pending_model.mesh_descriptor.clone(),
                        pending_model.instances.clone(),
                        pending_model.model.clone(),
                        material,
                        device,
                    );
                    renderables_ready.push(renderable);
                }
            }
        }

        renderables_ready
    }

    pub async fn load_model(
        &mut self,
        mesh_descriptor: ObjectWithMaterial,
        instances: &Vec<SceneComponent>,
        device: &Device,
    ) -> anyhow::Result<()> {
        let model = match &mesh_descriptor.mesh_source {
            MeshSource::PrimitiveInCode(shape) => self.primitive_shapes.get(shape).unwrap().clone(),
            MeshSource::FromFile(path) => {
                let mut file_buf_reader = open_file_for_reading(Path::new(path))?;
                Rc::new(load_obj(&mut file_buf_reader, &device, path.clone()).await?)
            }
        };

        let next_id = self.next_material_id;
        self.next_material_id += 1;

        self.pending_models.insert(
            next_id,
            PendingModel {
                model,
                material: PendingMaterialData::new(HashSet::new()),
                instances: instances.clone(),
                mesh_descriptor: mesh_descriptor.clone(),
            },
        );

        match &mesh_descriptor.material_descriptor {
            PbrMaterialDescriptor::Texture(textures) => {
                for texture_descriptor in textures {
                    let texture_id = self.queue_texture_for_loading(texture_descriptor, next_id);
                    if let Some(id) = texture_id {
                        self.texture_id_to_model_id.insert(id, next_id);
                    }
                }
            }
            PbrMaterialDescriptor::Flat(_) => todo!(),
        }

        Ok(())
    }

    //     pub async fn load_asset_file(
    //         &mut self,
    //         asset_name: &str,
    //         device: &wgpu::Device,
    //     ) -> anyhow::Result<(Primitive, u32)> {
    //         let asset_data = process_asset_file(asset_name)?;

    //         let mut file_buf_reader = open_file_for_reading(&asset_data.path)?;
    //         let model = load_obj(
    //             &mut file_buf_reader,
    //             &device,
    //             &asset_name.into(),
    //             asset_data.path.to_str().unwrap().to_owned(),
    //         )
    //         .await?;
    //         let pending_textures = asset_data
    //             .textures
    //             .into_iter()
    //             .map(|(texture_usage, path)| PendingTextureData {
    //                 texture_descriptor: TextureSourceDescriptor {
    //                     file_name: path,
    //                     usage: texture_usage,
    //                 },
    //             })
    //             .collect();

    //         let material_id = self.queue_material_for_loading(pending_textures);

    //         Ok((model, material_id))
    //     }
}

pub fn open_file_for_reading(file_path: &Path) -> anyhow::Result<BufReader<File>> {
    let file = File::open(file_path)?;
    Ok(BufReader::new(file))
}

// fn resolve_resource(resource_name: &str) -> anyhow::Result<PathBuf> {
//     let mut current_dir = env::current_dir()?;
//     current_dir.push(ASSETS_FOLDER_NAME);
//     current_dir.push(resource_name);
//     Ok(current_dir)
//     // Path::new(env!("OUT_DIR"))
//     //     .join("assets")
//     //     .join(resource_name)
// }

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

/// Reads the asset file and returns the path to the model and the texture files
// fn process_asset_file(asset_name: &str) -> anyhow::Result<ModelLoadingData> {
//     let model_folder = resolve_resource(asset_name)?;
//     let json_string = block_on(fs::read_to_string(model_folder.join(ASSET_FILE_NAME)))?;
//     let model_info: ModelDescriptorFile = serde_json::from_str(&json_string)?;

//     Ok(ModelLoadingData {
//         path: model_folder.join(model_info.model),
//         textures: model_info
//             .textures
//             .into_iter()
//             .map(|(texture_type, name)| {
//                 (
//                     texture_type,
//                     model_folder.join(TEXTURES_FOLDER_NAME).join(name),
//                 )
//             })
//             .collect::<Vec<_>>(),
//     })
// }

pub async fn load_obj<Reader>(
    reader: &mut Reader,
    device: &wgpu::Device,
    asset_path: String,
) -> anyhow::Result<Primitive>
where
    Reader: BufRead,
{
    let (mut models, _obj_materials) =
        tobj::load_obj_buf_async(reader, &tobj::GPU_LOAD_OPTIONS, |_| async {
            // We don't care about the mtl file, so this is just a dummy loader implementation
            MTLLoadResult::Ok((Default::default(), Default::default()))
        })
        .await?;

    let model = models.remove(0);

    Ok(Primitive::new(
        device,
        asset_path,
        vec_to_vec3s(model.mesh.positions),
        vec_to_vec3s(model.mesh.normals),
        vec_to_vec2s(model.mesh.texcoords),
        model.mesh.indices,
    ))
}
