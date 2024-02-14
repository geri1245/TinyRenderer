use std::fs::{self, File};
use std::io::BufReader;
use std::path::PathBuf;
use std::rc::Rc;

use glam::{Vec2, Vec3};

use crate::model;
use crate::model::{Material, ModelDescriptorFile, TextureData};
use crate::texture::{self, TextureUsage};

const ASSET_FILE_NAME: &str = "asset.json";

pub async fn open_file_for_reading(file_path: &PathBuf) -> anyhow::Result<BufReader<File>> {
    let file = File::open(file_path)?;
    Ok(BufReader::new(file))
}

pub async fn load_texture(
    file_name: &PathBuf,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    usage: TextureUsage,
) -> anyhow::Result<texture::SampledTexture> {
    let data = std::fs::read(file_name).unwrap();
    texture::SampledTexture::from_bytes(
        device,
        queue,
        &data,
        usage,
        file_name.file_name().unwrap().to_str().unwrap(),
    )
}

fn resolve_resource(resource_name: &str) -> PathBuf {
    std::path::Path::new(env!("OUT_DIR"))
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

pub async fn load_model<'a>(
    asset_name: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> anyhow::Result<model::Model> {
    let model_folder = resolve_resource(asset_name);
    let json_string = fs::read_to_string(model_folder.join(ASSET_FILE_NAME))?;
    let model_info: ModelDescriptorFile = serde_json::from_str(&json_string)?;

    let mut file_buf_reader = open_file_for_reading(&model_folder.join(&model_info.model)).await?;

    let (models, _obj_materials) =
        tobj::load_obj_buf_async(&mut file_buf_reader, &tobj::GPU_LOAD_OPTIONS, |p| async {
            let material_path = model_folder.join(p);
            let mut material_file_buf_reader = open_file_for_reading(&material_path).await.unwrap();
            tobj::load_mtl_buf(&mut material_file_buf_reader)
        })
        .await?;

    let mut textures = Vec::new();
    for (texture_type, texture_name) in model_info.textures {
        if texture_name.is_empty() {
            continue;
        }
        let texture_usage = match texture_type {
            model::TextureType::Albedo => TextureUsage::ALBEDO,
            model::TextureType::Normal => TextureUsage::NORMAL,
        };
        let material_path = model_folder.join("textures").join(&texture_name);
        let texture = load_texture(&material_path, device, queue, texture_usage).await?;
        let texture_data = TextureData {
            name: texture_name,
            texture: texture,
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
