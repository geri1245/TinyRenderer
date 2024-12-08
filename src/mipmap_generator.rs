use crate::{
    bind_group_layout_descriptors::{
        self, COMPUTE_SHADER_SDR_TEXTURE_DESTINATION, COMPUTE_SHADER_TEXTURE_WITH_SAMPLER,
    },
    pipelines::{ShaderCompilationSuccess, SimpleCP},
    texture::SampledTexture,
};
use wgpu::{
    BindGroup, CommandEncoder, ComputePassDescriptor, Device, Sampler, Texture, TextureDimension,
};

const MIP_MAP_GENERATOR_SHADER_SOURCE: &'static str = "src/shaders/mipmap_generator.wgsl";

const WORKGROUP_SIZE_PER_DIMENSION: u32 = 8;

/// Data for a single mip level when generating mip levels
struct MipLevelConfig {
    source_bind_group: BindGroup,
    destination_bind_group: BindGroup,
}

pub struct MipMapGenerator {
    mip_map_generator_pipeline: SimpleCP,
}

impl MipMapGenerator {
    pub async fn new(device: &Device) -> Self {
        let mip_map_generator_pipeline = SimpleCP::new(
            device,
            &[
                &bind_group_layout_descriptors::COMPUTE_SHADER_TEXTURE_WITH_SAMPLER,
                &bind_group_layout_descriptors::COMPUTE_SHADER_SDR_TEXTURE_DESTINATION,
            ],
            MIP_MAP_GENERATOR_SHADER_SOURCE,
            "mipmap generator",
        )
        .await
        .unwrap();

        Self {
            mip_map_generator_pipeline,
        }
    }

    pub async fn try_recompile_shader(
        &mut self,
        device: &wgpu::Device,
    ) -> anyhow::Result<ShaderCompilationSuccess> {
        self.mip_map_generator_pipeline
            .try_recompile_shader(device)
            .await
    }

    fn create_mip_generator_bind_groups(
        device: &Device,
        texture: &Texture,
        sampler: &Sampler,
        mip_count: u32,
    ) -> Vec<MipLevelConfig> {
        (0..mip_count)
            .map(|mip_level| {
                let texture_view = texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some(&format!("texture view for mip level {mip_level}")),
                    format: None,
                    dimension: None,
                    aspect: wgpu::TextureAspect::All,
                    base_mip_level: mip_level,
                    mip_level_count: Some(1),
                    base_array_layer: 0,
                    array_layer_count: None,
                });

                // When this mip level is the source and we filter it
                let source_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &device.create_bind_group_layout(&COMPUTE_SHADER_TEXTURE_WITH_SAMPLER),
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&texture_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&sampler),
                        },
                    ],
                    label: Some("mip generation source texture bind group"),
                });

                // When this mip level is the destination, eg. we are writing into this texture
                let destination_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &device
                        .create_bind_group_layout(&COMPUTE_SHADER_SDR_TEXTURE_DESTINATION),
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&texture_view),
                    }],
                    label: Some("mip generation destination texture bind group"),
                });

                MipLevelConfig {
                    source_bind_group,
                    destination_bind_group,
                }
            })
            .collect::<Vec<_>>()
    }

    pub fn create_mips_for_texture<'a>(
        &'a self,
        encoder: &mut CommandEncoder,
        texture: &SampledTexture,
        num_of_mips_to_generate: Option<u32>,
        device: &Device,
    ) {
        let original_texture_extensts = &texture.descriptor.extents;
        let num_of_mips_to_generate = num_of_mips_to_generate
            .unwrap_or(texture.descriptor.extents.max_mips(TextureDimension::D2));

        // Don't go over the max allocated mip level in the texture
        let mip_count = num_of_mips_to_generate.min(texture.descriptor.mip_count);

        let mip_configs = Self::create_mip_generator_bind_groups(
            device,
            &texture.texture,
            &texture.sampler,
            mip_count,
        );

        let mut target_texture_extents = (
            original_texture_extensts.width,
            original_texture_extensts.height,
        );
        for target_mip_level in 1..mip_count {
            let source_mip_level = target_mip_level - 1;
            target_texture_extents = (target_texture_extents.0 / 2, target_texture_extents.1 / 2);

            let num_dispatches_x = target_texture_extents
                .0
                .div_ceil(WORKGROUP_SIZE_PER_DIMENSION);
            let num_dispatches_y = target_texture_extents
                .1
                .div_ceil(WORKGROUP_SIZE_PER_DIMENSION);
            let invocation_dimensions = (num_dispatches_x, num_dispatches_y, 1);

            let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Mipmap generator compute pass"),
                timestamp_writes: None,
            });

            self.mip_map_generator_pipeline.run_copmute_pass(
                &mut compute_pass,
                &[
                    &mip_configs[source_mip_level as usize].source_bind_group,
                    &mip_configs[target_mip_level as usize].destination_bind_group,
                ],
                invocation_dimensions,
            );
        }
    }
}
