use wgpu::{
    CommandEncoder, CommandEncoderDescriptor, Device, Extent3d, InstanceDescriptor, RenderPass,
    RenderPassDepthStencilAttachment, SurfaceTexture, TextureFormat,
};

use crate::{
    color,
    texture::{self, SampledTexture},
    CLEAR_COLOR,
};

pub const MAX_LIGHTS: usize = 10;

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
    pub surface_texture_format: TextureFormat,

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
        let surface_texture_format = surface_capabilities.formats[0];

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
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

        Renderer {
            surface,
            device,
            queue,
            config,
            size,
            depth_texture,
            surface_texture_format,
            clear_color: color::wgpu_color_to_f32_array_rgba(CLEAR_COLOR),
        }
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
