use std::fs::{self, File};
use std::io::BufReader;
use std::path::PathBuf;
use std::rc::Rc;

use crate::model::{Material, ModelDescriptorFile, TextureType};
use crate::texture;
use crate::{bind_group_layout_descriptors, model};

const ASSET_FILE_NAME: &str = "asset.json";

pub async fn open_file_for_reading(file_path: &PathBuf) -> anyhow::Result<BufReader<File>> {
    let file = File::open(file_path)?;
    Ok(BufReader::new(file))
}

pub async fn load_texture(
    file_name: &PathBuf,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> anyhow::Result<texture::SampledTexture> {
    let data = std::fs::read(file_name).unwrap();
    texture::SampledTexture::from_bytes(
        device,
        queue,
        &data,
        file_name.file_name().unwrap().to_str().unwrap(),
    )
}

fn resolve_resource(resource_name: &str) -> PathBuf {
    std::path::Path::new(env!("OUT_DIR"))
        .join("assets")
        .join(resource_name)
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

    let mut possible_texture_sources = Vec::new();
    for texture_set in model_info.textures {
        possible_texture_sources.push((TextureType::Normal, texture_set.normal));
        possible_texture_sources.push((TextureType::Albedo, texture_set.albedo));
        possible_texture_sources.push((TextureType::Metal, texture_set.metalness));
        possible_texture_sources.push((TextureType::Rough, texture_set.roughness));
    }

    let mut material = Material::new();
    for (texture_type, texture_name) in possible_texture_sources {
        if texture_name.is_empty() {
            continue;
        }

        let material_path = model_folder.join("textures").join(&texture_name);
        let diffuse_texture = load_texture(&material_path, device, queue).await?;
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &device
                .create_bind_group_layout(&bind_group_layout_descriptors::STANDARD_TEXTURE),
            entries: &[
                diffuse_texture.get_texture_bind_group_entry(0),
                diffuse_texture.get_sampler_bind_group_entry(1),
            ],
            label: None,
        });

        material.add_texture(
            texture_type.clone(),
            model::TextureData {
                texture_type,
                name: texture_name,
                texture: diffuse_texture,
                bind_group,
            },
        );
    }

    let material = Rc::new(material);

    let meshes = models
        .into_iter()
        .map(
            |m| {
                model::Mesh::new(
                    device,
                    asset_name.to_string(),
                    m.mesh.positions,
                    m.mesh.normals,
                    m.mesh.texcoords,
                    m.mesh.indices,
                    material.clone(),
                )
            }, //      {
               //     name: asset_name.to_string(),
               //     vertex_buffer,
               //     index_buffer,
               //     index_count: m.mesh.indices.len() as u32,
               //     material: material.clone(),
               // }
        )
        .collect::<Vec<_>>();

    Ok(model::Model { meshes })
}
