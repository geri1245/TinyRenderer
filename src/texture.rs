use std::{fs::File, io::BufReader};

use anyhow::*;
use serde::{Deserialize, Serialize};
use wgpu::{Extent3d, TextureFormat, TextureUsages};

const SKYBOX_TEXTURE_SIZE: u32 = 512;

#[derive(Debug)]
pub struct SampledTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,

    pub descriptor: SampledTextureDescriptor,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum MaterialSource {
    FromFile(String),
    Defaults(TextureUsage),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TextureSourceDescriptor {
    pub source: MaterialSource,
    pub usage: TextureUsage,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SampledTextureDescriptor {
    pub format: TextureFormat,
    pub usages: TextureUsages,
    pub extents: Extent3d,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone, Copy)]
pub enum TextureUsage {
    Albedo,
    Normal,
    Metalness,
    Roughness,
    HdrAlbedo,
}

impl SampledTexture {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn get_texture_bind_group_entry<'a>(
        &'a self,
        binding_index: u32,
    ) -> wgpu::BindGroupEntry<'a> {
        wgpu::BindGroupEntry {
            binding: binding_index,
            resource: wgpu::BindingResource::TextureView(&self.view),
        }
    }

    pub fn get_sampler_bind_group_entry<'a>(
        &'a self,
        binding_index: u32,
    ) -> wgpu::BindGroupEntry<'a> {
        wgpu::BindGroupEntry {
            binding: binding_index,
            resource: wgpu::BindingResource::Sampler(&self.sampler),
        }
    }

    pub fn from_image_bytes(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
        usage: TextureUsage,
        label: Option<&str>,
    ) -> Result<Self> {
        let img = image::load_from_memory(bytes)?;
        let rgba = img.to_rgba8();
        let size = Extent3d {
            width: img.width(),
            height: img.height(),
            depth_or_array_layers: 1,
        };

        match usage {
            TextureUsage::Metalness | TextureUsage::Roughness => {
                let data = rgba
                    .into_vec()
                    .chunks_exact(4)
                    .map(|a| a[0] as f32 / 255.0)
                    .collect::<Vec<_>>();
                Self::from_image(
                    device,
                    queue,
                    bytemuck::cast_slice(&data),
                    size,
                    usage,
                    label,
                )
            }
            TextureUsage::HdrAlbedo => panic!("Hdr not supported in this function"),
            TextureUsage::Albedo | TextureUsage::Normal => {
                let data = &rgba.into_vec();
                Self::from_image(device, queue, data, size, usage, label)
            }
        }
    }

    pub fn from_hdr_image(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        path: &str,
        label: Option<&str>,
    ) -> Result<Self> {
        let f = File::open(path)?;
        let f = BufReader::new(f);
        let image = radiant::load(f)?;
        let mut bytes = Vec::new();
        for rgba in image.data {
            bytes.push(rgba.r);
            bytes.push(rgba.g);
            bytes.push(rgba.b);
            bytes.push(1.0); // Add an alpha value, as we can't have a 3 channel float texture
        }

        let texture_size = Extent3d {
            width: image.width as u32,
            height: image.height as u32,
            depth_or_array_layers: 1,
        };

        Self::from_image(
            device,
            queue,
            bytemuck::cast_slice(&bytes),
            texture_size,
            TextureUsage::HdrAlbedo,
            label,
        )
    }

    pub fn from_image(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
        size: Extent3d,
        usage: TextureUsage,
        label: Option<&str>,
    ) -> Result<Self> {
        let format = match usage {
            TextureUsage::Albedo => wgpu::TextureFormat::Rgba8Unorm,
            TextureUsage::Normal => wgpu::TextureFormat::Rgba8Unorm,
            TextureUsage::Metalness => wgpu::TextureFormat::R32Float,
            TextureUsage::Roughness => wgpu::TextureFormat::R32Float,
            TextureUsage::HdrAlbedo => wgpu::TextureFormat::Rgba32Float,
        };

        let bytes_per_pixel = match format {
            wgpu::TextureFormat::Rgba32Float => 4 * 4,
            wgpu::TextureFormat::R32Float => 4,
            _ => 4,
        };

        let gpu_usage = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST;

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: gpu_usage,
            view_formats: &[],
        });

        queue.write_texture(
            texture.as_image_copy(),
            bytes,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_pixel * size.width),
                rows_per_image: None,
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Ok(Self {
            texture,
            view,
            sampler,
            descriptor: SampledTextureDescriptor {
                format,
                extents: size,
                usages: gpu_usage,
            },
        })
    }

    pub fn create_depth_texture(
        device: &wgpu::Device,
        extent: wgpu::Extent3d,
        label: &str,
    ) -> Self {
        let gpu_usage =
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let desc = wgpu::TextureDescriptor {
            label: Some(label),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: gpu_usage,
            view_formats: &[],
        };
        let texture = device.create_texture(&desc);

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        Self {
            texture,
            view,
            sampler,
            descriptor: SampledTextureDescriptor {
                extents: extent,
                format: Self::DEPTH_FORMAT,
                usages: gpu_usage,
            },
        }
    }

    pub fn create_skybox_texture(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        // Is in the order in which the wgpu cubemap expects it: posX negX posY negY posZ negZ
        let images = vec![
            "assets/skybox/posX.png",
            "assets/skybox/negX.png",
            "assets/skybox/posY.png",
            "assets/skybox/negY.png",
            "assets/skybox/posZ.png",
            "assets/skybox/negZ.png",
        ];

        let size = wgpu::Extent3d {
            width: SKYBOX_TEXTURE_SIZE,
            height: SKYBOX_TEXTURE_SIZE,
            depth_or_array_layers: 6,
        };

        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let gpu_usage = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST;

        let texture_descriptor = wgpu::TextureDescriptor {
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: gpu_usage,
            label: None,
            view_formats: &[],
        };

        let texture = device.create_texture(&texture_descriptor);

        let mut bytes = Vec::new();
        for image_name in images {
            let file = File::open(image_name).unwrap();
            let image = image::load(BufReader::new(file), image::ImageFormat::Png).unwrap();
            bytes.extend_from_slice(&image.to_rgba8());
        }

        let image_copy_texture = texture.as_image_copy();
        queue.write_texture(
            image_copy_texture,
            &bytes,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * SKYBOX_TEXTURE_SIZE),
                rows_per_image: Some(SKYBOX_TEXTURE_SIZE),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..wgpu::TextureViewDescriptor::default()
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: None,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            texture,
            view,
            sampler,
            descriptor: SampledTextureDescriptor {
                extents: size,
                format,
                usages: gpu_usage,
            },
        }
    }

    pub fn new(device: &wgpu::Device, descriptor: SampledTextureDescriptor, label: &str) -> Self {
        let desc = wgpu::TextureDescriptor {
            label: Some(label),
            size: descriptor.extents,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: descriptor.format,
            usage: descriptor.usages,
            view_formats: &[],
        };
        let texture = device.create_texture(&desc);

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            texture,
            view,
            sampler,
            descriptor,
        }
    }
}
