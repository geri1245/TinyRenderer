use std::{collections::HashMap, rc::Rc};

use crate::{
    bind_group_layout_descriptors,
    buffer::{
        create_bind_group_from_buffer_entire_binding_init, BufferInitBindGroupCreationOptions,
    },
    model::PbrParameters,
    pipelines::PbrParameterVariation,
    texture::{SampledTexture, TextureUsage},
};

pub struct Material {
    pub bind_group: wgpu::BindGroup,
    pub variation: PbrParameterVariation,
}

impl Material {
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

        Material {
            bind_group,
            variation: PbrParameterVariation::Texture,
        }
    }

    pub fn from_flat_parameters(device: &wgpu::Device, pbr_parameters: &PbrParameters) -> Self {
        let (_buffer, bind_group) = create_bind_group_from_buffer_entire_binding_init(
            device,
            &BufferInitBindGroupCreationOptions {
                bind_group_layout_descriptor:
                    &bind_group_layout_descriptors::BUFFER_VISIBLE_EVERYWHERE,
                usages: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                label: "Equirectangular projection viewprojs".into(),
            },
            bytemuck::cast_slice(&[*pbr_parameters]),
        );

        Self {
            bind_group,
            variation: PbrParameterVariation::Flat,
        }
    }
}
