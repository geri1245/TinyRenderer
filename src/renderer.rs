use std::{fs::File, io::Write, time::Duration};
use wgpu::{
    CommandEncoder, InstanceDescriptor, RenderPass, RenderPassDepthStencilAttachment,
    SurfaceTexture, TextureFormat,
};

use crate::{
    camera_controller::CameraController,
    color,
    gui::GUI_PARAMS,
    light_controller::LightController,
    texture::{self, Texture},
    world::World,
};

pub const MAX_LIGHTS: usize = 10;

struct BufferDimensions {
    width: usize,
    height: usize,
    unpadded_bytes_per_row: usize,
    padded_bytes_per_row: usize,
}

impl BufferDimensions {
    fn new(width: usize, height: usize) -> Self {
        let bytes_per_pixel = std::mem::size_of::<u32>();
        let unpadded_bytes_per_row = width * bytes_per_pixel;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
        let padded_bytes_per_row_padding = (align - unpadded_bytes_per_row % align) % align;
        let padded_bytes_per_row = unpadded_bytes_per_row + padded_bytes_per_row_padding;
        Self {
            width,
            height,
            unpadded_bytes_per_row,
            padded_bytes_per_row,
        }
    }
}

struct OutputBuffer {
    dimensions: BufferDimensions,
    buffer: wgpu::Buffer,
    texture_extent: wgpu::Extent3d,
}

impl OutputBuffer {
    fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        // It is a WebGPU requirement that ImageCopyBuffer.layout.bytes_per_row % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT == 0
        // So we calculate padded_bytes_per_row by rounding unpadded_bytes_per_row
        // up to the next multiple of wgpu::COPY_BYTES_PER_ROW_ALIGNMENT.
        // https://en.wikipedia.org/wiki/Data_structure_alignment#Computing_padding
        let dimensions = BufferDimensions::new(width as usize, height as usize);
        // The output buffer lets us retrieve the data as an array
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Buffer to copy frame content into"),
            size: (dimensions.padded_bytes_per_row * dimensions.height) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let texture_extent = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        OutputBuffer {
            dimensions,
            buffer,
            texture_extent,
        }
    }
}

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
    pub surface_texture_format: TextureFormat,

    surface: wgpu::Surface<'static>,

    depth_texture: texture::Texture,

    frame_content_copy_dest: OutputBuffer,

    should_capture_frame_content: bool,
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

        let output_buffer = OutputBuffer::new(&device, config.width, config.height);

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
            frame_content_copy_dest: output_buffer,
            should_capture_frame_content: false,
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
        self.frame_content_copy_dest =
            OutputBuffer::new(&self.device, new_size.width, new_size.height);
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
        let submission_index = self.queue.submit(Some(encoder.finish()));

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

    pub fn render(
        &mut self,
        window: &winit::window::Window,
        camera_controller: &CameraController,
        light_controller: &LightController,
        world: &World,
        delta: Duration,
    ) -> Result<(), wgpu::SurfaceError> {
        // encoder.copy_texture_to_buffer(
        //     output.texture.as_image_copy(),
        //     wgpu::ImageCopyBuffer {
        //         buffer: &self.frame_content_copy_dest.buffer,
        //         layout: wgpu::ImageDataLayout {
        //             offset: 0,
        //             bytes_per_row: Some(
        //                 self.frame_content_copy_dest.dimensions.padded_bytes_per_row as u32,
        //             ),
        //             rows_per_image: None,
        //         },
        //     },
        //     self.frame_content_copy_dest.texture_extent,
        // );

        // Draw GUI

        // if self.should_draw_gui {
        //     self.gui.render(
        //         &window,
        //         &self.device,
        //         &self.queue,
        //         delta,
        //         &view,
        //         self.gui_params.clone(),
        //     );
        // }

        // if self.should_capture_frame_content {
        //     self.should_capture_frame_content = false;
        //     let future = self.create_png("./frame.png", submission_index);
        //     block_on(future);
        // }

        Ok(())
    }

    async fn create_png(&self, png_output_path: &str, submission_index: wgpu::SubmissionIndex) {
        let buffer_slice = self.frame_content_copy_dest.buffer.slice(..);
        // Sets the buffer up for mapping, sending over the result of the mapping back to us when it is finished.
        let (sender, receiver) = futures_intrusive::channel::shared::oneshot_channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());

        // TODO: Either Poll without blocking or move the blocking polling to another thread
        self.device
            .poll(wgpu::Maintain::WaitForSubmissionIndex(submission_index));

        let has_file_system_available = cfg!(not(target_arch = "wasm32"));
        if !has_file_system_available {
            return;
        }

        if let Some(Ok(())) = receiver.receive().await {
            let padded_buffer = buffer_slice.get_mapped_range();

            let mut png_encoder = png::Encoder::new(
                File::create(png_output_path).unwrap(),
                self.frame_content_copy_dest.dimensions.width as u32,
                self.frame_content_copy_dest.dimensions.height as u32,
            );
            png_encoder.set_depth(png::BitDepth::Eight);
            png_encoder.set_color(png::ColorType::Rgba);
            let mut png_writer = png_encoder
                .write_header()
                .unwrap()
                .into_stream_writer_with_size(
                    self.frame_content_copy_dest
                        .dimensions
                        .unpadded_bytes_per_row,
                )
                .unwrap();

            // from the padded_buffer we write just the unpadded bytes into the image
            for chunk in
                padded_buffer.chunks(self.frame_content_copy_dest.dimensions.padded_bytes_per_row)
            {
                png_writer
                    .write_all(
                        &chunk[..self
                            .frame_content_copy_dest
                            .dimensions
                            .unpadded_bytes_per_row],
                    )
                    .unwrap();
            }
            png_writer.finish().unwrap();

            // With the current interface, we have to make sure all mapped views are
            // dropped before we unmap the buffer.
            drop(padded_buffer);

            self.frame_content_copy_dest.buffer.unmap();
        }
    }

    pub fn toggle_should_draw_gui(&mut self) {
        self.should_draw_gui = !self.should_draw_gui
    }
}
