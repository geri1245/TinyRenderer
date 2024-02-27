use std::fs::File;
use std::{collections::HashMap, thread::Thread};

use async_std::{
    fs,
    path::{Path, PathBuf},
    task::block_on,
};
use image::DynamicImage;
use std::io::BufReader;
use tobj::MTLLoadResult;
use wgpu::Device;

use glam::{Vec2, Vec3};

use crossbeam_channel::{Receiver, Sender};
use rayon::ThreadPool;

use crate::model::{self, ModelLoadingData, RenderableMesh, TextureType};
use crate::model::{ModelDescriptorFile, TextureData};
use crate::texture::{self, TextureUsage};

const ASSET_FILE_NAME: &str = "asset.json";
const TEXTURES_FOLDER_NAME: &str = "textures";

const MAX_WORKER_COUNT: usize = 4;

pub struct FileLoadStatus {
    pub id: u32,
    pub loaded_image: DynamicImage,
}

pub struct AssetLoader {
    next_resource_id: u32,
    thread_pool: ThreadPool,
    result_sender: Sender<anyhow::Result<FileLoadStatus>>,
    result_receiver: Receiver<anyhow::Result<FileLoadStatus>>,
}

impl AssetLoader {
    pub fn new() -> Self {
        let thread_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(MAX_WORKER_COUNT)
            .build()
            .unwrap();
        let (result_sender, result_receiver) = crossbeam_channel::unbounded();

        AssetLoader {
            next_resource_id: 0,
            thread_pool,
            result_sender,
            result_receiver,
        }
    }

    fn try_load_data(path: PathBuf) -> anyhow::Result<DynamicImage> {
        let data = block_on(fs::read(path))?;
        let img = image::load_from_memory(&data)?;
        Ok(img)
    }

    pub fn start_loading_bytes(&mut self, path: &PathBuf) -> u32 {
        let resource_id = self.next_resource_id;
        self.next_resource_id += 1;
        let path = path.clone();
        let result_sender = self.result_sender.clone();

        // TODO: don't swallow the errors, propagate them
        self.thread_pool.spawn(move || {
            let image = Self::try_load_data(path).map(|image| FileLoadStatus {
                id: resource_id,
                loaded_image: image,
            });
            result_sender.send(image).unwrap();
        });

        resource_id
    }

    pub fn poll_loading_resources(&self) -> Option<Vec<FileLoadStatus>> {
        let mut completed_resource_loads = Vec::new();
        while let Ok(result) = self.result_receiver.try_recv() {
            completed_resource_loads.push(result.unwrap());
        }

        if completed_resource_loads.is_empty() {
            None
        } else {
            Some(completed_resource_loads)
        }
    }
}

struct PendingTextureData {
    file_name: PathBuf,
    usage: TextureUsage,
}

pub struct ResourceLoader {
    asset_loader: AssetLoader,
    loading_id_to_asset_data: HashMap<u32, PendingTextureData>,
    pending_materials: Vec<u32>,
}

impl ResourceLoader {
    pub fn new() -> Self {
        let asset_loader = AssetLoader::new();
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
    ) -> Vec<(u32, TextureType, TextureData)> {
        let results = self
            .asset_loader
            .poll_loading_resources()
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
                    device,
                    queue,
                    &asset_load_result.loaded_image,
                    pending_texture_data.usage,
                    Some(file_name),
                )
                .unwrap();

                (
                    asset_load_result.id,
                    match pending_texture_data.usage {
                        TextureUsage::Albedo => TextureType::Albedo,
                        TextureUsage::Normal => TextureType::Normal,
                    },
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
            .map(|(texture_type, path)| {
                let texture_usage = match texture_type {
                    model::TextureType::Albedo => TextureUsage::Albedo,
                    model::TextureType::Normal => TextureUsage::Normal,
                };
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
