use async_std::task::block_on;
use wgpu::{
    CommandEncoder, CommandEncoderDescriptor, InstanceDescriptor, MemoryHints, SurfaceTexture,
    TextureFormat,
};

use crate::{mipmap_generator::MipMapGenerator, pipelines::ShaderCompilationSuccess};

pub const MAX_LIGHTS: usize = 10;

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
    pub surface_texture_format: TextureFormat,

    pub mip_map_generator: MipMapGenerator,

    surface: wgpu::Surface<'static>,
}

impl Renderer {
    pub fn new(window: &winit::window::Window) -> Renderer {
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
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .unwrap();

        let supported_features = adapter.features();
        let required_features = wgpu::Features::DEPTH_CLIP_CONTROL
            | wgpu::Features::TEXTURE_FORMAT_16BIT_NORM
            | wgpu::Features::FLOAT32_FILTERABLE;
        if !supported_features.contains(required_features) {
            panic!("Not all required features are supported. \nRequired features: {:?}\nSupported features: {:?}", required_features, supported_features);
        }
        let (device, queue) = block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                required_features,
                // WebGL doesn't support all of wgpu's features, so if
                // we're building for the web we'll have to disable some.
                required_limits: if cfg!(target_arch = "wasm32") {
                    wgpu::Limits::downlevel_webgl2_defaults()
                } else {
                    wgpu::Limits {
                        max_bind_groups: 8,
                        ..Default::default()
                    }
                },
                label: None,
                memory_hints: MemoryHints::Performance,
            },
            None,
        ))
        .unwrap();

        let surface_capabilities = surface.get_capabilities(&adapter);
        // TODO: Unfortunately copying from an rgba to a bgra texture is not supported
        // At the same time having a bgra texture as a storage attachment (to the post processing
        // pipeline) is also not supported
        // So if we want to be able to copy the post processing texture to the framebuffer, then we have
        // to use rgba here (even though bgra8unormsrgb seemed to be the preferred format on my system)
        let surface_texture_format = TextureFormat::Rgba8Unorm;
        if !surface_capabilities
            .formats
            .contains(&surface_texture_format)
        {
            panic!(
                "Format {:?} is not supported as the main render target",
                surface_texture_format
            );
        }

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            format: surface_texture_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoNoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let mip_map_generator = MipMapGenerator::new(&device);

        Renderer {
            surface,
            device,
            queue,
            config,
            size,
            surface_texture_format,
            mip_map_generator,
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.size = new_size;
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);
    }

    pub fn get_encoder(&self) -> CommandEncoder {
        self.device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            })
    }

    pub fn get_current_frame_texture(&self) -> Result<SurfaceTexture, wgpu::SurfaceError> {
        self.surface.get_current_texture()
    }

    pub fn try_recompile_shaders(&mut self) -> anyhow::Result<ShaderCompilationSuccess> {
        self.mip_map_generator.try_recompile_shader(&self.device)
    }
}
