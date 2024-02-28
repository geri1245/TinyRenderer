use wgpu::{
    BindGroup, BindGroupDescriptor, CommandEncoder, CommandEncoderDescriptor, Device, Extent3d,
    InstanceDescriptor, RenderPass, RenderPassDepthStencilAttachment, SurfaceTexture,
    TextureFormat,
};

use crate::{
    bind_group_layout_descriptors::{COMPUTE_RENDER_TO_FRAMEBUFFER, STANDARD_TEXTURE},
    color,
    texture::{self, SampledTexture, SampledTextureDescriptor},
    CLEAR_COLOR,
};

pub const MAX_LIGHTS: usize = 10;

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
    pub surface_texture_format: TextureFormat,
    pub full_screen_render_target_ping_pong_textures: Vec<SampledTexture>,
    pub compute_bind_group_target: BindGroup,
    pub compute_bind_group_source: BindGroup,

    surface: wgpu::Surface<'static>,

    depth_texture: texture::SampledTexture,
    clear_color: [f32; 4],
}

impl Renderer {
    pub async fn new(window: &winit::window::Window) -> Renderer {
        let size = window.inner_size();

        // Backends::all => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(InstanceDescriptor {
            backends: wgpu::Backends::DX12,
            ..Default::default()
        });
        let surface = unsafe {
            instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::from_window(window).unwrap())
                .unwrap()
        };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let supported_features = adapter.features();
        let required_features =
            wgpu::Features::DEPTH_CLIP_CONTROL | wgpu::Features::TEXTURE_FORMAT_16BIT_NORM;
        if !supported_features.contains(required_features) {
            panic!("Not all required features are supported. \nRequired features: {:?}\nSupported features: {:?}", required_features, supported_features);
        }
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    required_features: required_features,
                    // WebGL doesn't support all of wgpu's features, so if
                    // we're building for the web we'll have to disable some.
                    required_limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    } else {
                        wgpu::Limits::default()
                    },
                    label: None,
                },
                None,
            )
            .await
            .unwrap();

        let device = device;
        let queue = queue;

        let surface_capabilities = surface.get_capabilities(&adapter);
        // TODO: Unfortunately copying from an rgba to a bgra texture is not supported
        // At the same time having a bgra texture as a storage attachment (to the post processing
        // pipeline) is also not supported
        // So if we want to be able to copy the post processing texture to the framebuffer, then we have
        // to use rgba here (even though bgra8unormsrgb seemed to be the preferred format on my system)
        let surface_texture_format = TextureFormat::Rgba8Unorm;
        if !surface_capabilities
            .formats
            .contains(&TextureFormat::Rgba8Unorm)
        {
            panic!("Format {:?} is not supported", surface_texture_format);
        }

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            format: surface_texture_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let depth_texture = Renderer::create_depth_texture(&device, config.width, config.height);

        let (textures, bind_group_target, bind_group_source) =
            Self::create_pingpong_texture(&device, config.width, config.height);

        Renderer {
            surface,
            device,
            queue,
            config,
            size,
            depth_texture,
            surface_texture_format,
            clear_color: color::wgpu_color_to_f32_array_rgba(CLEAR_COLOR),
            full_screen_render_target_ping_pong_textures: textures,
            compute_bind_group_target: bind_group_target,
            compute_bind_group_source: bind_group_source,
        }
    }

    fn create_pingpong_texture(
        device: &Device,
        width: u32,
        height: u32,
    ) -> (Vec<SampledTexture>, BindGroup, BindGroup) {
        let full_screen_render_target_ping_pong_textures = (0..2)
            .map(|_| {
                let texture = SampledTexture::new(
                    &device,
                    &SampledTextureDescriptor {
                        width,
                        height,
                        usages: wgpu::TextureUsages::STORAGE_BINDING
                            | wgpu::TextureUsages::RENDER_ATTACHMENT
                            | wgpu::TextureUsages::COPY_SRC
                            | wgpu::TextureUsages::COPY_DST
                            | wgpu::TextureUsages::TEXTURE_BINDING,
                        format: TextureFormat::Rgba8Unorm,
                    },
                    "PingPong texture for postprocessing",
                );
                texture
            })
            .collect::<Vec<_>>();

        let bind_group_for_target = {
            let layout = device.create_bind_group_layout(&COMPUTE_RENDER_TO_FRAMEBUFFER);

            device.create_bind_group(&BindGroupDescriptor {
                label: Some("Bind group of the destination/source os the postprocess pipeline"),
                entries: &[
                    full_screen_render_target_ping_pong_textures[1].get_texture_bind_group_entry(0)
                ],
                layout: &layout,
            })
        };

        let bind_group_for_source = {
            let layout = device.create_bind_group_layout(&STANDARD_TEXTURE);

            device.create_bind_group(&BindGroupDescriptor {
                label: Some("Bind group of the destination/source os the postprocess pipeline"),
                entries: &[
                    full_screen_render_target_ping_pong_textures[0].get_texture_bind_group_entry(0),
                    full_screen_render_target_ping_pong_textures[0].get_sampler_bind_group_entry(1),
                ],
                layout: &layout,
            })
        };

        (
            full_screen_render_target_ping_pong_textures,
            bind_group_for_target,
            bind_group_for_source,
        )
    }

    fn create_depth_texture(device: &Device, width: u32, height: u32) -> SampledTexture {
        SampledTexture::create_depth_texture(
            device,
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            "Main depth texture",
        )
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.size = new_size;
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);
        self.depth_texture =
            Renderer::create_depth_texture(&self.device, self.config.width, self.config.height);

        let (textures, bind_group_target, bind_group_source) =
            Self::create_pingpong_texture(&self.device, self.config.width, self.config.height);

        self.full_screen_render_target_ping_pong_textures = textures;
        self.compute_bind_group_target = bind_group_target;
        self.compute_bind_group_source = bind_group_source;
    }

    pub fn begin_frame<'a>(&'a self) -> CommandEncoder {
        self.device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            })
    }

    pub fn get_current_frame_texture(&self) -> Result<SurfaceTexture, wgpu::SurfaceError> {
        self.surface.get_current_texture()
    }

    pub fn end_frame(&self, output_frame_content: SurfaceTexture) {
        output_frame_content.present();
    }

    pub fn begin_main_render_pass<'a>(
        &'a self,
        encoder: &'a mut CommandEncoder,
        view: &'a wgpu::TextureView,
        depth_texture_view: &'a wgpu::TextureView,
    ) -> RenderPass<'a> {
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render pass that uses the GBuffer"),
            timestamp_writes: None,
            occlusion_query_set: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(color::f32_array_rgba_to_wgpu_color(
                        self.clear_color,
                    )),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: depth_texture_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
        })
    }
}
