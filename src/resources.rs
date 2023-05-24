use std::io::{BufReader, Cursor};
use std::rc::Rc;
use wgpu::util::DeviceExt;

use crate::{bind_group_layout_descriptors, model};
use crate::{texture, vertex};

pub async fn load_string(file_name: &str) -> anyhow::Result<String> {
    #[cfg(target_arch = "wasm32")]
    {
        let url = format_url(file_name);
        let txt = reqwest::get(url).await?.text().await?;
        Ok(txt)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = std::path::Path::new(env!("OUT_DIR"))
            .join("assets")
            .join(file_name);
        let txt = std::fs::read_to_string(path)?;
        Ok(txt)
    }
}

pub async fn load_binary(file_name: &str) -> anyhow::Result<Vec<u8>> {
    #[cfg(target_arch = "wasm32")]
    {
        let url = format_url(file_name);
        reqwest::get(url).await?.bytes().await?.to_vec()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = std::path::Path::new(env!("OUT_DIR"))
            .join("assets")
            .join(file_name);
        Ok(std::fs::read(path)?)
    }
}

pub async fn load_texture(
    file_name: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> anyhow::Result<texture::Texture> {
    let data = load_binary(file_name).await?;
    texture::Texture::from_bytes(device, queue, &data, file_name)
}

pub async fn load_model<'a>(
    file_name: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> anyhow::Result<model::Model> {
    let obj_text = load_string(file_name).await?;
    let obj_cursor = Cursor::new(obj_text);
    let mut obj_reader = BufReader::new(obj_cursor);

    let (models, obj_materials) = tobj::load_obj_buf_async(
        &mut obj_reader,
        &tobj::LoadOptions {
            triangulate: true,
            single_index: true,
            ..Default::default()
        },
        |p| async move {
            let mat_text = load_string(&p).await.unwrap();
            tobj::load_mtl_buf(&mut BufReader::new(Cursor::new(mat_text)))
        },
    )
    .await?;

    let mut materials = Vec::new();
    for m in obj_materials? {
        let diffuse_texture = load_texture(&m.diffuse_texture, device, queue).await?;
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &device
                .create_bind_group_layout(&bind_group_layout_descriptors::DIFFUSE_TEXTURE),
            entries: &[
                diffuse_texture.get_texture_bind_group_entry(0),
                diffuse_texture.get_sampler_bind_group_entry(1),
            ],
            label: None,
        });

        materials.push(Rc::new(model::Material {
            name: m.name,
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
                label: Some(&format!("{:?} Vertex Buffer", file_name)),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{:?} Index Buffer", file_name)),
                contents: bytemuck::cast_slice(&m.mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            model::Mesh {
                name: file_name.to_string(),
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
