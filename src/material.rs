use std::{collections::HashMap, rc::Rc};

use wgpu::RenderPass;

use crate::{
    bind_group_layout_descriptors,
    buffer::{
        create_bind_group_from_buffer_entire_binding_init, BufferInitBindGroupCreationOptions,
    },
    model::PbrParameters,
    texture::{SampledTexture, TextureSourceDescriptor, TextureUsage},
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum PbrMaterialDescriptor {
    Texture(Vec<TextureSourceDescriptor>), // All the parameters are given as textures
    Flat(PbrParameters),                   // The parameters are given as plain old numbers
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
            label: None,
        });

        MaterialRenderData { bind_group }
    }

    pub fn from_flat_parameters(device: &wgpu::Device, pbr_parameters: &PbrParameters) -> Self {
        let (_buffer, bind_group) = create_bind_group_from_buffer_entire_binding_init(
            device,
            &BufferInitBindGroupCreationOptions {
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
