use std::fs::{self, File};

use async_std::path::{Path, PathBuf};
use std::io::BufReader;
use std::rc::Rc;
use tobj::MTLLoadResult;

use glam::{Vec2, Vec3};

use crate::model::{self, ModelLoadingData};
use crate::model::{Material, ModelDescriptorFile, TextureData};
use crate::texture::{self, TextureUsage};

const ASSET_FILE_NAME: &str = "asset.json";
const TEXTURES_FOLDER_NAME: &str = "textures";

pub fn open_file_for_reading(file_path: &PathBuf) -> anyhow::Result<BufReader<File>> {
    let file = File::open(file_path)?;
    Ok(BufReader::new(file))
}

pub async fn load_texture(
    file_name: &PathBuf,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    usage: TextureUsage,
) -> anyhow::Result<texture::SampledTexture> {
    let data = async_std::fs::read(file_name).await.unwrap();
    texture::SampledTexture::from_bytes(
        device,
        queue,
        &data,
        usage,
        file_name.file_name().unwrap().to_str().unwrap(),
    )
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
    let json_string = fs::read_to_string(model_folder.join(ASSET_FILE_NAME))?;
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

pub async fn load_model<'a>(
    asset_name: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> anyhow::Result<model::Model> {
    let asset_data = process_asset_file(asset_name)?;

    let mut file_buf_reader = open_file_for_reading(&asset_data.model)?;

    let (models, _obj_materials) =
        tobj::load_obj_buf_async(&mut file_buf_reader, &tobj::GPU_LOAD_OPTIONS, |_| async {
            // We don't care about the mtl file, so this is just a dummy loader implementation
            MTLLoadResult::Ok((Default::default(), Default::default()))
        })
        .await?;

    let mut textures = Vec::new();
    for (texture_type, texture_path) in asset_data.textures {
        let texture_usage = match texture_type {
            model::TextureType::Albedo => TextureUsage::Albedo,
            model::TextureType::Normal => TextureUsage::Normal,
        };
        let texture = load_texture(&texture_path, device, queue, texture_usage).await?;
        let texture_data = TextureData {
            name: texture_path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned(),
            texture,
            texture_type: texture_type.clone(),
        };
        textures.push((texture_type, texture_data))
    }

    let material = Rc::new(Material::new(device, textures));

    let meshes = models
        .into_iter()
        .map(|m| {
            model::Mesh::new(
                device,
                asset_name.to_string(),
                vec_to_vec3s(m.mesh.positions),
                vec_to_vec3s(m.mesh.normals),
                vec_to_vec2s(m.mesh.texcoords),
                m.mesh.indices,
                material.clone(),
            )
        })
        .collect::<Vec<_>>();

    Ok(model::Model { meshes })
}

// pub fn load_material() -> Material {}
