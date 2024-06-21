use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::rc::Rc;

use async_std::{
    fs,
    path::{Path, PathBuf},
    task::block_on,
};
use std::io::{BufRead, BufReader};
use tobj::MTLLoadResult;
use wgpu::{Device, Extent3d};

use glam::{Vec2, Vec3};

use crate::material::Material;
use crate::model::ModelDescriptorFile;
use crate::texture::{self, SampledTexture, TextureUsage};
use crate::{
    file_loader::FileLoader,
    model::{self, ModelLoadingData, RenderableMesh},
};

const ASSET_FILE_NAME: &str = "asset.json";
const TEXTURES_FOLDER_NAME: &str = "textures";

#[derive(Debug, Hash, PartialEq, Eq)]
pub enum PrimitiveShape {
    Cube,
}

struct PendingTextureData {
    file_name: PathBuf,
    usage: TextureUsage,
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
    ) -> Rc<Material> {
        for (usage, texture) in default_textures {
            if !self.textures.contains_key(&usage) {
                self.textures.insert(*usage, texture.clone());
            }
        }

        Rc::new(Material::new(device, &self.textures))
    }
}

pub struct ResourceLoader {
    asset_loader: FileLoader,
    loading_id_to_asset_data: HashMap<u32, PendingTextureData>,
    pending_materials: HashMap<u32, PendingMaterialData>,
    next_material_id: u32,
    texture_id_to_material_id: HashMap<u32, u32>,
    pub default_mat: Rc<Material>,
    default_textures: HashMap<TextureUsage, Rc<SampledTexture>>,
    primitive_shapes: HashMap<PrimitiveShape, Rc<RenderableMesh>>,
}

impl ResourceLoader {
    pub async fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let asset_loader = FileLoader::new();

        let (default_mat, default_textures) = Self::load_default_textures(device, queue);
        let primitive_shapes = Self::load_primitive_shapes(device).await.unwrap();

        let loader = ResourceLoader {
            asset_loader,
            loading_id_to_asset_data: HashMap::new(),
            pending_materials: HashMap::new(),
            next_material_id: 0,
            texture_id_to_material_id: HashMap::new(),
            default_mat,
            default_textures,
            primitive_shapes,
        };

        loader
    }

    pub fn get_default_material(&self) -> Rc<Material> {
        self.default_mat.clone()
    }

    pub fn get_primitive_shape(&self, shape: PrimitiveShape) -> Rc<RenderableMesh> {
        self.primitive_shapes.get(&shape).unwrap().clone()
    }

    async fn load_primitive_shapes(
        device: &Device,
    ) -> anyhow::Result<HashMap<PrimitiveShape, Rc<RenderableMesh>>> {
        let bytes: Vec<u8> = include_bytes!("../assets/models/cube/cube.obj").into();
        let mut reader = BufReader::new(&bytes[..]);
        let mesh = Rc::new(load_obj(&mut reader, device, &"cube".into(), None).await?);

        let mut primitive_shapes = HashMap::new();
        primitive_shapes.insert(PrimitiveShape::Cube, mesh);

        return Ok(primitive_shapes);
    }

    fn load_default_textures(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> (Rc<Material>, HashMap<TextureUsage, Rc<SampledTexture>>) {
        const TEXTURES: [(&[u8], &'static str, texture::TextureUsage); 4] = [
            (
                include_bytes!("../assets/textures/defaults/albedo.png"),
                "default albedo texture",
                texture::TextureUsage::Albedo,
            ),
            (
                include_bytes!("../assets/textures/defaults/normal.png"),
                "default normal texture",
                texture::TextureUsage::Normal,
            ),
            (
                include_bytes!("../assets/textures/defaults/metalness.png"),
                "default metalness texture",
                texture::TextureUsage::Metalness,
            ),
            (
                include_bytes!("../assets/textures/defaults/roughness.png"),
                "default roughness texture",
                texture::TextureUsage::Roughness,
            ),
        ];

        let mut default_material_textures = HashMap::new();

        for (data, name, usage) in TEXTURES {
            let texture = Rc::new(
                texture::SampledTexture::from_image_bytes(device, queue, data, usage, Some(name))
                    .unwrap(),
            );
            default_material_textures.insert(usage, texture);
        }

        (
            Rc::new(Material::new(device, &default_material_textures)),
            default_material_textures,
        )
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

            let texture_size = Extent3d {
                width: asset_load_result.loaded_image.width(),
                height: asset_load_result.loaded_image.height(),
                depth_or_array_layers: 1,
            };
            let texture = Rc::new(
                texture::SampledTexture::from_image(
                    &device,
                    queue,
                    &asset_load_result.loaded_image,
                    texture_size,
                    pending_texture_data.usage,
                    Some(file_name),
                )
                .unwrap(),
            );

            let material_id = self
                .texture_id_to_material_id
                .get(&asset_load_result.id)
                .unwrap();
            let pending_material = self.pending_materials.get_mut(&material_id).unwrap();

            pending_material.add_texture(asset_load_result.id, pending_texture_data.usage, texture);

            if pending_material.is_ready() {
                materials_ready.push((
                    *material_id,
                    pending_material.get_material(device, &self.default_textures),
                ));
            }
        }

        materials_ready
    }

    pub async fn load_asset_file(
        &mut self,
        asset_name: &str,
        device: &wgpu::Device,
    ) -> anyhow::Result<(model::RenderableMesh, u32)> {
        let asset_data = process_asset_file(asset_name)?;

        let mut file_buf_reader = open_file_for_reading(&asset_data.path)?;
        let model = load_obj(
            &mut file_buf_reader,
            &device,
            &asset_name.into(),
            Some(asset_data.path.to_str().unwrap().to_owned()),
        )
        .await?;
        let pending_textures = asset_data
            .textures
            .into_iter()
            .map(|(texture_usage, path)| PendingTextureData {
                file_name: path,
                usage: texture_usage,
            })
            .collect();

        let material_id = self.queue_material_for_loading(pending_textures);

        Ok((model, material_id))
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
        path: model_folder.join(model_info.model),
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

pub async fn load_obj<Reader>(
    reader: &mut Reader,
    device: &wgpu::Device,
    asset_name: &String,
    asset_path: Option<String>,
) -> anyhow::Result<RenderableMesh>
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

    Ok(model::RenderableMesh::new(
        device,
        asset_name.to_string(),
        asset_path,
        vec_to_vec3s(model.mesh.positions),
        vec_to_vec3s(model.mesh.normals),
        vec_to_vec2s(model.mesh.texcoords),
        model.mesh.indices,
    ))
}
