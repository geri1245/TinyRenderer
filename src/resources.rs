use std::fs::{self, File};
use std::io::BufReader;
use std::path::PathBuf;
use std::rc::Rc;
use wgpu::util::DeviceExt;

use crate::model::ModelDescriptorFile;
use crate::{bind_group_layout_descriptors, model};
use crate::{texture, vertex};

const ASSET_FILE_NAME: &str = "asset.json";

pub async fn open_file_for_reading(file_path: &PathBuf) -> anyhow::Result<BufReader<File>> {
    let file = File::open(file_path)?;
    Ok(BufReader::new(file))
}

pub async fn load_texture(
    file_name: &PathBuf,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> anyhow::Result<texture::Texture> {
    let data = std::fs::read(file_name).unwrap();
    texture::Texture::from_bytes(
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

    let (models, _obj_materials) = tobj::load_obj_buf_async(
        &mut file_buf_reader,
        &tobj::LoadOptions {
            triangulate: true,
            single_index: true,
            ..Default::default()
        },
        |p| async {
            let material_path = model_folder.join(p);
            let mut material_file_buf_reader = open_file_for_reading(&material_path).await.unwrap();
            tobj::load_mtl_buf(&mut material_file_buf_reader)
        },
    )
    .await?;

    let mut possible_texture_sources = Vec::new();
    for texture_set in model_info.textures {
        possible_texture_sources.push(texture_set.normal);
        possible_texture_sources.push(texture_set.albedo);
        possible_texture_sources.push(texture_set.metalness);
        possible_texture_sources.push(texture_set.roughness);
    }

    let mut materials = Vec::new();
    for texture_name in possible_texture_sources {
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

        materials.push(Rc::new(model::Material {
            name: texture_name,
            diffuse_texture,
            bind_group,
        }))
    }

    let meshes = models
        .into_iter()
        .map(|m| {
            let vertices = (0..m.mesh.positions.len() / 3)
                .map(|i| vertex::VertexRaw {
                    position: [
                        m.mesh.positions[i * 3],
                        m.mesh.positions[i * 3 + 1],
                        m.mesh.positions[i * 3 + 2],
                    ],
                    tex_coord: [m.mesh.texcoords[i * 2], m.mesh.texcoords[i * 2 + 1]],
                    normal: [
                        m.mesh.normals[i * 3],
                        m.mesh.normals[i * 3 + 1],
                        m.mesh.normals[i * 3 + 2],
                    ],
                })
                .collect::<Vec<_>>();

            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{:?} Vertex Buffer", asset_name)),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{:?} Index Buffer", asset_name)),
                contents: bytemuck::cast_slice(&m.mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            model::Mesh {
                name: asset_name.to_string(),
                vertex_buffer,
                index_buffer,
                index_count: m.mesh.indices.len() as u32,
                material: m
                    .mesh
                    .material_id
                    .map(|material_index| materials[material_index].clone()),
            }
        })
        .collect::<Vec<_>>();

    Ok(model::Model { meshes, materials })
}
