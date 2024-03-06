use wgpu::{BindGroup, BindGroupDescriptor, ComputePass, Device, TextureFormat};

use crate::{
    bind_group_layout_descriptors::{COMPUTE_FINAL_STAGE, COMPUTE_PING_PONG},
    pipelines::{PostProcessPipelineTargetTextureVariant, PostProcessRP},
    texture::{SampledTexture, SampledTextureDescriptor},
};

const POSTPROCESS_TEXTURE_FORMAT: TextureFormat = TextureFormat::Rgba16Float;
const WORKGROUP_SIZE_PER_DIMENSION: u32 = 8;

pub struct PostProcessManager {
    pub pipeline: PostProcessRP,

    pub full_screen_render_target_ping_pong_textures: Vec<SampledTexture>,
    pub compute_bind_group_0_to_1: BindGroup,
    pub compute_bind_group_1_to_0: BindGroup,
}

impl PostProcessManager {
    pub async fn new(device: &Device, width: u32, height: u32) -> Self {
        let pipeline =
            PostProcessRP::new(device, PostProcessPipelineTargetTextureVariant::Rgba8Unorm)
                .await
                .unwrap();
        let (textures, bind_group_0_to_1, bind_group_1_to_0) =
            Self::create_pingpong_texture(&device, width, height);

        Self {
            pipeline,
            full_screen_render_target_ping_pong_textures: textures,
            compute_bind_group_0_to_1: bind_group_0_to_1,
            compute_bind_group_1_to_0: bind_group_1_to_0,
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let (textures, bind_group_0_to_1, bind_group_1_to_0) =
            Self::create_pingpong_texture(device, width, height);

        self.full_screen_render_target_ping_pong_textures = textures;
        self.compute_bind_group_0_to_1 = bind_group_0_to_1;
        self.compute_bind_group_1_to_0 = bind_group_1_to_0;
    }

    fn create_pingpong_texture(
        device: &Device,
        width: u32,
        height: u32,
    ) -> (Vec<SampledTexture>, BindGroup, BindGroup) {
        let full_screen_render_target_ping_pong_textures = (0..2)
            .map(|i| {
                let mut usages = wgpu::TextureUsages::STORAGE_BINDING
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::TEXTURE_BINDING;
                if i == 0 {
                    usages |= wgpu::TextureUsages::RENDER_ATTACHMENT;
                }
                let texture_format = if i == 0 {
                    POSTPROCESS_TEXTURE_FORMAT
                } else {
                    TextureFormat::Rgba8Unorm
                };

                let texture = SampledTexture::new(
                    &device,
                    &SampledTextureDescriptor {
                        width,
                        height,
                        usages,
                        format: texture_format,
                    },
                    "PingPong texture for postprocessing",
                );
                texture
            })
            .collect::<Vec<_>>();

        let bind_group_1_to_0 = {
            let layout = device.create_bind_group_layout(&COMPUTE_PING_PONG);

            device.create_bind_group(&BindGroupDescriptor {
                label: Some("Bind group of the destination/source of the postprocess pipeline"),
                entries: &[
                    full_screen_render_target_ping_pong_textures[0].get_texture_bind_group_entry(0),
                    full_screen_render_target_ping_pong_textures[1].get_texture_bind_group_entry(1),
                    full_screen_render_target_ping_pong_textures[1].get_sampler_bind_group_entry(2),
                ],
                layout: &layout,
            })
        };

        let bind_group_0_to_1 = {
            let layout = device.create_bind_group_layout(&COMPUTE_FINAL_STAGE);

            device.create_bind_group(&BindGroupDescriptor {
                label: Some("Bind group of the destination/source of the postprocess pipeline"),
                entries: &[
                    full_screen_render_target_ping_pong_textures[1].get_texture_bind_group_entry(0),
                    full_screen_render_target_ping_pong_textures[0].get_texture_bind_group_entry(1),
                    full_screen_render_target_ping_pong_textures[0].get_sampler_bind_group_entry(2),
                ],
                layout: &layout,
            })
        };

        (
            full_screen_render_target_ping_pong_textures,
            bind_group_0_to_1,
            bind_group_1_to_0,
        )
    }

    pub fn render<'a>(
        &'a self,
        compute_pass: &mut ComputePass<'a>,
        render_target_width: u32,
        render_target_height: u32,
    ) {
        let num_dispatches_x = render_target_width.div_ceil(WORKGROUP_SIZE_PER_DIMENSION);
        let num_dispatches_y = render_target_height.div_ceil(WORKGROUP_SIZE_PER_DIMENSION);
        self.pipeline.run_copmute_pass(
            compute_pass,
            &self.compute_bind_group_0_to_1,
            (num_dispatches_x, num_dispatches_y, 1),
        );
    }
}
