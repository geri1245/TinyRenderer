use wgpu::{
    BindGroup, BindGroupDescriptor, ComputePass, Device, Extent3d, TextureDimension, TextureFormat,
};

use crate::{
    bind_group_layout_descriptors::{self, COMPUTE_FINAL_STAGE, COMPUTE_PING_PONG},
    pipelines::{ShaderCompilationSuccess, SimpleCP},
    texture::{SampledTexture, SampledTextureDescriptor},
};

const POST_PROCESS_SHADER_SOURCE: &'static str = "src/shaders/post_process.wgsl";
const SCREEN_SPACE_REFLECTION_SHADER_SOURCE: &'static str =
    "src/shaders/screen_space_reflection.wgsl";
const TONE_MAPPING_SHADER_SOURCE: &'static str = "src/shaders/tone_mapping.wgsl";

const POSTPROCESS_TEXTURE_FORMAT: TextureFormat = TextureFormat::Rgba16Float;
const WORKGROUP_SIZE_PER_DIMENSION: u32 = 8;

pub struct PostProcessManager {
    dummy_pipeline: SimpleCP,
    screen_space_reflection_pipeline: SimpleCP,
    tone_mapping_pipeline: SimpleCP,

    pub full_screen_render_target_ping_pong_textures: Vec<SampledTexture>,
    pub compute_bind_group_0_to_1: BindGroup,
    pub compute_bind_group_1_to_0: BindGroup,
    pub tone_mapping_bind_group: BindGroup,
}

impl PostProcessManager {
    pub async fn new(device: &Device, width: u32, height: u32) -> Self {
        let dummy_pipeline = SimpleCP::new(
            device,
            &[
                &bind_group_layout_descriptors::COMPUTE_PING_PONG,
                &bind_group_layout_descriptors::BUFFER_VISIBLE_EVERYWHERE,
            ],
            POST_PROCESS_SHADER_SOURCE,
            "dummy",
        )
        .await
        .unwrap();

        let screen_space_reflection_pipeline = SimpleCP::new(
            device,
            &[
                &bind_group_layout_descriptors::COMPUTE_PING_PONG,
                &bind_group_layout_descriptors::BUFFER_VISIBLE_EVERYWHERE,
            ],
            SCREEN_SPACE_REFLECTION_SHADER_SOURCE,
            "screen space reflections",
        )
        .await
        .unwrap();

        let tone_mapping_pipeline = SimpleCP::new(
            device,
            &[
                &bind_group_layout_descriptors::COMPUTE_FINAL_STAGE,
                &bind_group_layout_descriptors::BUFFER_VISIBLE_EVERYWHERE,
            ],
            TONE_MAPPING_SHADER_SOURCE,
            "tone mapping",
        )
        .await
        .unwrap();

        let (textures, bind_group_0_to_1, bind_group_1_to_0, tone_mapping_bind_group) =
            Self::create_pingpong_texture(&device, width, height);

        Self {
            dummy_pipeline,
            screen_space_reflection_pipeline,
            tone_mapping_pipeline,
            full_screen_render_target_ping_pong_textures: textures,
            compute_bind_group_0_to_1: bind_group_0_to_1,
            compute_bind_group_1_to_0: bind_group_1_to_0,
            tone_mapping_bind_group,
        }
    }

    pub async fn try_recompile_shader(
        &mut self,
        device: &wgpu::Device,
    ) -> anyhow::Result<ShaderCompilationSuccess> {
        self.dummy_pipeline.try_recompile_shader(device).await?;
        self.screen_space_reflection_pipeline
            .try_recompile_shader(device)
            .await?;
        self.tone_mapping_pipeline
            .try_recompile_shader(device)
            .await
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let (textures, bind_group_0_to_1, bind_group_1_to_0, tone_mapping_bind_group) =
            Self::create_pingpong_texture(device, width, height);

        self.full_screen_render_target_ping_pong_textures = textures;
        self.compute_bind_group_0_to_1 = bind_group_0_to_1;
        self.compute_bind_group_1_to_0 = bind_group_1_to_0;
        self.tone_mapping_bind_group = tone_mapping_bind_group;
    }

    fn create_pingpong_texture(
        device: &Device,
        width: u32,
        height: u32,
    ) -> (Vec<SampledTexture>, BindGroup, BindGroup, BindGroup) {
        let full_screen_render_target_ping_pong_textures = (0..3)
            .map(|i| {
                let mut usages = wgpu::TextureUsages::STORAGE_BINDING
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::TEXTURE_BINDING;
                if i == 0 {
                    usages |= wgpu::TextureUsages::RENDER_ATTACHMENT;
                }

                let texture_format = if i == 2 {
                    // We need to be able to copy from one of the textures to the screen render target and its format is
                    // this one
                    TextureFormat::Rgba8Unorm
                } else {
                    POSTPROCESS_TEXTURE_FORMAT
                };

                let texture = SampledTexture::new(
                    &device,
                    SampledTextureDescriptor {
                        usages,
                        format: texture_format,
                        extents: Extent3d {
                            width,
                            height,
                            depth_or_array_layers: 1,
                        },
                        dimension: TextureDimension::D2,
                        mip_count: 1,
                    },
                    &format!("PingPong texture for postprocessing {i}"),
                );
                texture
            })
            .collect::<Vec<_>>();

        let bind_group_1_to_0 = {
            let layout = device.create_bind_group_layout(&COMPUTE_PING_PONG);

            device.create_bind_group(&BindGroupDescriptor {
                label: Some(
                    "Bind group of the destination/source of the postprocess pipeline 1 to 0",
                ),
                entries: &[
                    full_screen_render_target_ping_pong_textures[0].get_texture_bind_group_entry(0),
                    full_screen_render_target_ping_pong_textures[1].get_texture_bind_group_entry(1),
                    full_screen_render_target_ping_pong_textures[1].get_sampler_bind_group_entry(2),
                ],
                layout: &layout,
            })
        };

        let bind_group_0_to_1 = {
            let layout = device.create_bind_group_layout(&COMPUTE_PING_PONG);

            device.create_bind_group(&BindGroupDescriptor {
                label: Some(
                    "Bind group of the destination/source of the postprocess pipeline 0 to 1",
                ),
                entries: &[
                    full_screen_render_target_ping_pong_textures[1].get_texture_bind_group_entry(0),
                    full_screen_render_target_ping_pong_textures[0].get_texture_bind_group_entry(1),
                    full_screen_render_target_ping_pong_textures[0].get_sampler_bind_group_entry(2),
                ],
                layout: &layout,
            })
        };

        let tone_mapping_bind_group = {
            let layout = device.create_bind_group_layout(&COMPUTE_FINAL_STAGE);

            device.create_bind_group(&BindGroupDescriptor {
                label: Some("Tone mapping bind group"),
                entries: &[
                    full_screen_render_target_ping_pong_textures[2].get_texture_bind_group_entry(0),
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
            tone_mapping_bind_group,
        )
    }

    pub fn render<'a>(
        &'a self,
        compute_pass: &mut ComputePass<'a>,
        render_target_width: u32,
        render_target_height: u32,
        global_gpu_params_bind_group: &'a BindGroup,
    ) {
        let num_dispatches_x = render_target_width.div_ceil(WORKGROUP_SIZE_PER_DIMENSION);
        let num_dispatches_y = render_target_height.div_ceil(WORKGROUP_SIZE_PER_DIMENSION);
        let invocation_dimensions = (num_dispatches_x, num_dispatches_y, 1);

        self.dummy_pipeline.run_copmute_pass(
            compute_pass,
            &[
                &self.compute_bind_group_0_to_1,
                global_gpu_params_bind_group,
            ],
            invocation_dimensions,
        );

        self.screen_space_reflection_pipeline.run_copmute_pass(
            compute_pass,
            &[
                &self.compute_bind_group_1_to_0,
                global_gpu_params_bind_group,
            ],
            invocation_dimensions,
        );

        self.tone_mapping_pipeline.run_copmute_pass(
            compute_pass,
            &[&self.tone_mapping_bind_group, global_gpu_params_bind_group],
            invocation_dimensions,
        );
    }
}
