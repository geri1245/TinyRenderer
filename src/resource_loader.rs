use std::collections::HashMap;
use std::fs::File;

use async_std::{
    fs,
    path::{Path, PathBuf},
    task::block_on,
};
use std::io::BufReader;
use tobj::MTLLoadResult;
use wgpu::Device;

use glam::{Vec2, Vec3};

use crate::model::{ModelDescriptorFile, TextureData};
use crate::texture::{self, TextureUsage};
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

pub struct ResourceLoader {
    asset_loader: FileLoader,
    loading_id_to_asset_data: HashMap<u32, PendingTextureData>,
    pending_materials: Vec<u32>,
}

impl ResourceLoader {
    pub fn new() -> Self {
        let asset_loader = FileLoader::new();
        // let channels = crossbeam_channel::unbounded();
        ResourceLoader {
            asset_loader,
            loading_id_to_asset_data: HashMap::new(),
            pending_materials: Vec::new(),
        }
    }

    fn queue_texture_for_loading(&mut self, texture_to_load: PendingTextureData) -> u32 {
        let loading_id = self
            .asset_loader
            .start_loading_bytes(&texture_to_load.file_name);
        self.loading_id_to_asset_data
            .insert(loading_id, texture_to_load);
        loading_id
    }

    pub fn poll_loaded_textures(
        &self,
        device: &Device,
        queue: &wgpu::Queue,
    ) -> Vec<(u32, TextureUsage, TextureData)> {
        let results = self
            .asset_loader
            .poll_loaded_resources()
            .unwrap_or_default();

        results
            .into_iter()
            .map(|asset_load_result| {
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

                (
                    asset_load_result.id,
                    pending_texture_data.usage,
                    TextureData {
                        name: file_name.into(),
                        texture,
                    },
                )
            })
            .collect::<Vec<_>>()
    }

    pub async fn load_asset_file(
        &mut self,
        asset_name: &str,
        device: &wgpu::Device,
    ) -> anyhow::Result<(model::RenderableMesh, Vec<u32>)> {
        let asset_data = process_asset_file(asset_name)?;

        let model = load_obj(&asset_data.model, &device, &asset_name.into()).await?;
        let loading_ids = asset_data
            .textures
            .into_iter()
            .map(|(texture_usage, path)| {
                self.queue_texture_for_loading(PendingTextureData {
                    file_name: path,
                    usage: texture_usage,
                })
            })
            .collect();
        Ok((model, loading_ids))
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
