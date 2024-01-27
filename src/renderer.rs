use wgpu::{
    CommandEncoder, InstanceDescriptor, RenderPass, RenderPassDepthStencilAttachment,
    SurfaceTexture, TextureFormat,
};

use crate::{
    color,
    gui::GUI_PARAMS,
    texture::{self, Texture},
};

pub const MAX_LIGHTS: usize = 10;

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
    pub surface_texture_format: TextureFormat,

    surface: wgpu::Surface<'static>,

    depth_texture: texture::Texture,

    should_draw_gui: bool,
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

        let depth_texture = texture::Texture::create_depth_texture(
            &device,
            config.width,
            config.height,
            "depth_texture",
        );

        Renderer {
            surface,
            device,
            queue,
            config,
            size,
            depth_texture,
            should_draw_gui: true,
            surface_texture_format,
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.size = new_size;
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);
        self.depth_texture = Texture::create_depth_texture(
            &self.device,
            self.config.width,
            self.config.height,
            "Depth texture",
        );
    }

    pub fn begin_frame(&self) -> CommandEncoder {
        self.device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            })
    }

    pub fn get_current_frame_texture(&self) -> Result<SurfaceTexture, wgpu::SurfaceError> {
        self.surface.get_current_texture()
    }

    pub fn end_frame(&self, encoder: CommandEncoder, output_frame_content: SurfaceTexture) {
        self.queue.submit(Some(encoder.finish()));

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
                        GUI_PARAMS.clear_color,
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

    pub fn toggle_should_draw_gui(&mut self) {
        self.should_draw_gui = !self.should_draw_gui
    }
}
