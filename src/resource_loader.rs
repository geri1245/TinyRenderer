use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::rc::Rc;

use async_std::{
    fs,
    path::{Path, PathBuf},
    task::block_on,
};
use std::io::BufReader;
use tobj::MTLLoadResult;
use wgpu::Device;

use glam::{Vec2, Vec3};

use crate::model::{Material, ModelDescriptorFile, TexturedRenderableMesh};
use crate::texture::{self, SampledTexture, TextureUsage};
use crate::{
    file_loader::FileLoader,
    model::{self, ModelLoadingData, RenderableMesh},
};

const ASSET_FILE_NAME: &str = "asset.json";
const TEXTURES_FOLDER_NAME: &str = "textures";

struct PendingTextureData {
    file_name: PathBuf,
    usage: TextureUsage,
}

struct PendingMaterialData {
    textures: HashMap<TextureUsage, SampledTexture>,
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
        texture: SampledTexture,
    ) {
        self.textures.insert(texture_usage, texture);
        self.missing_texture_load_ids.remove(&texture_load_id);
    }

    fn is_ready(&self) -> bool {
        self.missing_texture_load_ids.is_empty()
    }
}

pub struct ResourceLoader {
    asset_loader: FileLoader,
    loading_id_to_asset_data: HashMap<u32, PendingTextureData>,
    pending_materials: HashMap<u32, PendingMaterialData>,
    next_material_id: u32,
    texture_id_to_material_id: HashMap<u32, u32>,
    default_mat: Rc<Material>,
}

impl ResourceLoader {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let asset_loader = FileLoader::new();
        // let channels = crossbeam_channel::unbounded();

        let default_mat = Self::load_default_textures(device, queue);

        let loader = ResourceLoader {
            asset_loader,
            loading_id_to_asset_data: HashMap::new(),
            pending_materials: HashMap::new(),
            next_material_id: 0,
            texture_id_to_material_id: HashMap::new(),
            default_mat,
        };

        loader
    }

    fn load_default_textures(device: &wgpu::Device, queue: &wgpu::Queue) -> Rc<Material> {
        let default_normal_bytes = include_bytes!("../assets/defaults/normal.png");
        let default_normal_texture = texture::SampledTexture::from_bytes(
            device,
            queue,
            default_normal_bytes,
            texture::TextureUsage::Normal,
            "default normal texture",
        )
        .unwrap();
        let default_albedo_bytes = include_bytes!("../assets/defaults/albedo.png");
        let default_albedo_texture = texture::SampledTexture::from_bytes(
            device,
            queue,
            default_albedo_bytes,
            texture::TextureUsage::Albedo,
            "default albedo texture",
        )
        .unwrap();

        let mut default_material_textures = HashMap::new();
        default_material_textures.insert(TextureUsage::Albedo, default_albedo_texture);
        default_material_textures.insert(TextureUsage::Normal, default_normal_texture);

        Rc::new(Material::new(device, &default_material_textures))
    }

    fn queue_texture_for_loading(&mut self, texture_to_load: PendingTextureData) -> u32 {
        let loading_id = self
            .asset_loader
            .start_loading_bytes(&texture_to_load.file_name);
        self.loading_id_to_asset_data
            .insert(loading_id, texture_to_load);
        loading_id
    }

    fn queue_material_for_loading(&mut self, textures_in_material: Vec<PendingTextureData>) -> u32 {
        let current_material_id = self.next_material_id;
        self.next_material_id += 1;
        let mut pending_texture_ids = HashSet::new();

        for texture in textures_in_material {
            let loading_id = self.queue_texture_for_loading(texture);
            self.texture_id_to_material_id
                .insert(loading_id, current_material_id);
            pending_texture_ids.insert(loading_id);
        }

        self.pending_materials.insert(
            current_material_id,
            PendingMaterialData::new(pending_texture_ids),
        );

        current_material_id
    }

    pub fn poll_loaded_textures(
        &mut self,
        device: &Device,
        queue: &wgpu::Queue,
    ) -> Vec<(u32, Rc<Material>)> {
        let results = self
            .asset_loader
            .poll_loaded_resources()
            .unwrap_or_default();

        let mut materials_ready = Vec::new();

        for asset_load_result in results {
            // TODO: Error handling
            let pending_texture_data = self
                .loading_id_to_asset_data
                .get(&asset_load_result.id)
                .unwrap();

            let file_name = pending_texture_data
                .file_name
                .file_name()
                .unwrap()
                .to_str()
                .unwrap();
            let texture = texture::SampledTexture::from_image(
                &device,
                queue,
                &asset_load_result.loaded_image,
                pending_texture_data.usage,
                Some(file_name),
            )
            .unwrap();

            let material_id = self
                .texture_id_to_material_id
                .get(&asset_load_result.id)
                .unwrap();
            let pending_material = self.pending_materials.get_mut(&material_id).unwrap();

            pending_material.add_texture(asset_load_result.id, pending_texture_data.usage, texture);

            if pending_material.is_ready() {
                materials_ready.push((
                    *material_id,
                    Rc::new(Material::new(device, &pending_material.textures)),
                ));
            }
        }

        materials_ready
    }

    pub async fn load_asset_file(
        &mut self,
        asset_name: &str,
        device: &wgpu::Device,
    ) -> anyhow::Result<(model::TexturedRenderableMesh, u32)> {
        let asset_data = process_asset_file(asset_name)?;

        let model = load_obj(&asset_data.model, &device, &asset_name.into()).await?;
        let pending_textures = asset_data
            .textures
            .into_iter()
            .map(|(texture_usage, path)| PendingTextureData {
                file_name: path,
                usage: texture_usage,
            })
            .collect();

        let material_id = self.queue_material_for_loading(pending_textures);

        Ok((
            TexturedRenderableMesh {
                material: self.default_mat.clone(),
                mesh: model,
            },
            material_id,
        ))
    }
}

pub fn open_file_for_reading(file_path: &PathBuf) -> anyhow::Result<BufReader<File>> {
    let file = File::open(file_path)?;
    Ok(BufReader::new(file))
}

fn resolve_resource(resource_name: &str) -> PathBuf {
    Path::new(env!("OUT_DIR"))
        .join("assets")
        .join(resource_name)
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

/// Reads the asset file and returns the path to the model and the texture files
fn process_asset_file(asset_name: &str) -> anyhow::Result<ModelLoadingData> {
    let model_folder = resolve_resource(asset_name);
    let json_string = block_on(fs::read_to_string(model_folder.join(ASSET_FILE_NAME)))?;
    let model_info: ModelDescriptorFile = serde_json::from_str(&json_string)?;

    Ok(ModelLoadingData {
        model: model_folder.join(model_info.model),
        textures: model_info
            .textures
            .into_iter()
            .map(|(texture_type, name)| {
                (
                    texture_type,
                    model_folder.join(TEXTURES_FOLDER_NAME).join(name),
                )
            })
            .collect::<Vec<_>>(),
    })
}

pub async fn load_obj(
    path: &PathBuf,
    device: &wgpu::Device,
    asset_name: &String,
) -> anyhow::Result<RenderableMesh> {
    let mut file_buf_reader = open_file_for_reading(&path)?;
    let (mut models, _obj_materials) =
        tobj::load_obj_buf_async(&mut file_buf_reader, &tobj::GPU_LOAD_OPTIONS, |_| async {
            // We don't care about the mtl file, so this is just a dummy loader implementation
            MTLLoadResult::Ok((Default::default(), Default::default()))
        })
        .await?;

    let model = models.remove(0);

    Ok(model::RenderableMesh::new(
        device,
        asset_name.to_string(),
        vec_to_vec3s(model.mesh.positions),
        vec_to_vec3s(model.mesh.normals),
        vec_to_vec2s(model.mesh.texcoords),
        model.mesh.indices,
    ))
}
