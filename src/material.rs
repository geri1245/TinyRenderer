use std::{
    collections::{HashMap, HashSet},
    ops::Index,
    rc::Rc,
};

use glam::Vec3;
use wgpu::RenderPass;

use crate::{
    bind_group_layout_descriptors,
    buffer::{create_bind_group_from_buffer_entire_binding_init, GpuBufferCreationOptions},
    model::PbrParameters,
    renderer::Renderer,
    resource_loader::ResourceLoader,
    texture::{MaterialSource, SampledTexture, TextureSourceDescriptor, TextureUsage},
};

#[derive(
    Debug,
    Clone,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    ui_item_derive::UiDisplayable,
    ui_item_derive::UiSettableNew,
)]
pub enum PbrMaterialDescriptor {
    Texture(Vec<TextureSourceDescriptor>), // All the parameters are given as textures
    Flat(PbrParameters),                   // The parameters are given as plain old numbers
}

impl Default for PbrMaterialDescriptor {
    fn default() -> Self {
        Self::Flat(PbrParameters::default())
    }
}

impl PbrMaterialDescriptor {
    pub fn from_color(color: Vec3) -> Self {
        Self::Flat(PbrParameters::new(color, 1.0, 0.0))
    }
}

#[derive(Debug)]
pub struct MaterialRenderData {
    pub bind_group: wgpu::BindGroup,
}

impl MaterialRenderData {
    pub fn new(
        device: &wgpu::Device,
        textures: &HashMap<TextureUsage, Rc<SampledTexture>>,
    ) -> Self {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &device.create_bind_group_layout(&bind_group_layout_descriptors::PBR_TEXTURE),
            entries: &[
                textures
                    .get(&TextureUsage::Albedo)
                    .unwrap()
                    .get_texture_bind_group_entry(0),
                textures
                    .get(&TextureUsage::Albedo)
                    .unwrap()
                    .get_sampler_bind_group_entry(1),
                textures
                    .get(&TextureUsage::Normal)
                    .unwrap()
                    .get_texture_bind_group_entry(2),
                textures
                    .get(&TextureUsage::Normal)
                    .unwrap()
                    .get_sampler_bind_group_entry(3),
                textures
                    .get(&TextureUsage::Roughness)
                    .unwrap()
                    .get_texture_bind_group_entry(4),
                textures
                    .get(&TextureUsage::Roughness)
                    .unwrap()
                    .get_sampler_bind_group_entry(5),
                textures
                    .get(&TextureUsage::Metalness)
                    .unwrap()
                    .get_texture_bind_group_entry(6),
                textures
                    .get(&TextureUsage::Metalness)
                    .unwrap()
                    .get_sampler_bind_group_entry(7),
            ],
            label: Some("Pbr texture bind group"),
        });

        MaterialRenderData { bind_group }
    }

    pub fn from_textures(
        renderer: &Renderer,
        textures: &Vec<TextureSourceDescriptor>,
        resource_loader: &ResourceLoader,
    ) -> anyhow::Result<Self> {
        let mut texture_map = HashMap::new();

        let mut texture_usages = HashSet::new();
        texture_usages.insert(TextureUsage::Albedo);
        texture_usages.insert(TextureUsage::Normal);
        texture_usages.insert(TextureUsage::Roughness);
        texture_usages.insert(TextureUsage::Metalness);

        for texture_desc in textures {
            texture_usages.remove(&texture_desc.usage);
            let texture = resource_loader.load_texture(texture_desc, renderer)?;
            texture_map.insert(texture_desc.usage, texture);
        }

        for usage in texture_usages {
            if !texture_map.contains_key(&usage) {
                let texture = resource_loader.load_texture(
                    &TextureSourceDescriptor {
                        source: MaterialSource::Default,
                        usage,
                    },
                    renderer,
                )?;
                texture_map.insert(usage, texture);
            }
        }

        Ok(Self::new(&renderer.device, &texture_map))
    }

    pub fn from_flat_parameters(device: &wgpu::Device, pbr_parameters: &PbrParameters) -> Self {
        let (_buffer, bind_group) = create_bind_group_from_buffer_entire_binding_init(
            device,
            &GpuBufferCreationOptions {
                bind_group_layout_descriptor:
                    &bind_group_layout_descriptors::BUFFER_VISIBLE_EVERYWHERE,
                usages: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                label: "Flat PBR parameter buffer".into(),
            },
            bytemuck::cast_slice(&[*pbr_parameters]),
        );

        Self { bind_group }
    }

    // TODO: once it's possible to bind values by name, then do it that way
    pub fn bind_render_pass<'a>(
        &'a self,
        render_pass: &mut RenderPass<'a>,
        material_bind_group_index: u32,
    ) {
        render_pass.set_bind_group(material_bind_group_index, &self.bind_group, &[]);
    }
}
