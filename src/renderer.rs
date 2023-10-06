use async_std::task::block_on;
use std::{cell::RefCell, fs::File, io::Write, rc::Rc, time::Duration};
use wgpu::{InstanceDescriptor, RenderPassDepthStencilAttachment};

use crate::{
    camera_controller::CameraController,
    color,
    gui::{Gui, GuiParams},
    light_controller::LightController,
    pipelines,
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

    surface: wgpu::Surface,

    main_rp: pipelines::MainRP,
    forward_rp: pipelines::ForwardRP,
    skybox_rp: pipelines::SkyboxRP,
    shadow_rp: pipelines::ShadowRP,
    gbuffer_rp: pipelines::GBufferGeometryRP,

    depth_texture: texture::Texture,

    frame_content_copy_dest: OutputBuffer,

    should_capture_frame_content: bool,
    should_draw_gui: bool,

    gui: crate::gui::Gui,
    gui_params: Rc<RefCell<crate::gui::GuiParams>>,
}

impl Renderer {
    pub async fn new(
        window: &winit::window::Window,
        gui_params: Rc<RefCell<GuiParams>>,
    ) -> Renderer {
        let size = window.inner_size();

        // Backends::all => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(InstanceDescriptor {
            backends: wgpu::Backends::DX12,
            ..Default::default()
        });
        let surface = unsafe { instance.create_surface(window) }.unwrap();
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
                    features: required_features,
                    // WebGL doesn't support all of wgpu's features, so if
                    // we're building for the web we'll have to disable some.
                    limits: if cfg!(target_arch = "wasm32") {
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
        let format = surface_capabilities.formats[0];

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        let output_buffer = OutputBuffer::new(&device, config.width, config.height);

        let gui = Gui::new(&window, &device, &queue, format);

        let depth_texture = texture::Texture::create_depth_texture(
            &device,
            config.width,
            config.height,
            "depth_texture",
        );

        let main_rp = pipelines::MainRP::new(&device, config.format);
        let skybox_rp = pipelines::SkyboxRP::new(&device, &queue, config.format);
        let gbuffer_rp = pipelines::GBufferGeometryRP::new(&device, config.width, config.height);
        let forward_rp = pipelines::ForwardRP::new(&device, config.format);
        let shadow_rp = crate::pipelines::ShadowRP::new(&device);

        Renderer {
            surface,
            device,
            queue,
            config,
            size,
            main_rp,
            forward_rp,
            depth_texture,
            frame_content_copy_dest: output_buffer,
            should_capture_frame_content: false,
            should_draw_gui: true,
            gui,
            gui_params,
            shadow_rp,
            skybox_rp,
            gbuffer_rp,
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
        self.gbuffer_rp
            .resize(&self.device, new_size.width, new_size.height);
        self.frame_content_copy_dest =
            OutputBuffer::new(&self.device, new_size.width, new_size.height);
    }

    pub fn render(
        &mut self,
        window: &winit::window::Window,
        camera_controller: &CameraController,
        light_controller: &LightController,
        world: &World,
        delta: Duration,
    ) -> Result<(), wgpu::SurfaceError> {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        self.shadow_rp.render(
            &mut encoder,
            &world.obj_model,
            &light_controller.bind_group,
            world.instances.len(),
            &world.instance_buffer,
        );

        {
            let mut render_pass = self.gbuffer_rp.begin_render(&mut encoder);
            self.gbuffer_rp.render_model(
                &mut render_pass,
                &world.obj_model,
                &camera_controller.bind_group,
                world.instances.len(),
                &world.instance_buffer,
            );

            self.gbuffer_rp.render_mesh(
                &mut render_pass,
                &world.square,
                &camera_controller.bind_group,
                1,
                &world.square_instance_buffer,
            );
        }

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render pass that uses the GBuffer"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(color::f32_array_rgba_to_wgpu_color(
                            self.gui_params.borrow().clear_color,
                        )),
                        store: true,
                    },
                })],
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view: &self.gbuffer_rp.textures.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: false,
                    }),
                    stencil_ops: None,
                }),
            });

            {
                render_pass.push_debug_group("Cubes rendering from GBuffer");

                self.main_rp.render(
                    &mut render_pass,
                    camera_controller,
                    light_controller,
                    &self.gbuffer_rp.bind_group,
                    &self.shadow_rp.bind_group,
                );

                render_pass.pop_debug_group();
            }

            {
                render_pass.push_debug_group("Skybox rendering");

                self.skybox_rp.render(&mut render_pass, camera_controller);

                render_pass.pop_debug_group();
            }

            {
                render_pass.push_debug_group("Forward rendering light debug objects");
                self.forward_rp.render_model(
                    &mut render_pass,
                    &world.obj_model,
                    &camera_controller.bind_group,
                    &light_controller.bind_group,
                    1,
                    &light_controller.light_instance_buffer,
                );

                render_pass.pop_debug_group();
            }
        }

        encoder.copy_texture_to_buffer(
            output.texture.as_image_copy(),
            wgpu::ImageCopyBuffer {
                buffer: &self.frame_content_copy_dest.buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(
                        self.frame_content_copy_dest.dimensions.padded_bytes_per_row as u32,
                    ),
                    rows_per_image: None,
                },
            },
            self.frame_content_copy_dest.texture_extent,
        );

        let submission_index = self.queue.submit(Some(encoder.finish()));

        // Draw GUI

        if self.should_draw_gui {
            self.gui.render(
                &window,
                &self.device,
                &self.queue,
                delta,
                &view,
                self.gui_params.clone(),
            );
        }

        output.present();

        if self.should_capture_frame_content {
            self.should_capture_frame_content = false;
            let future = self.create_png("./frame.png", submission_index);
            block_on(future);
        }

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

    pub fn handle_event<'a, T>(
        &mut self,
        window: &winit::window::Window,
        event: &winit::event::Event<'a, T>,
    ) {
        self.gui.handle_event(window, event);
    }

    pub fn toggle_should_draw_gui(&mut self) {
        self.should_draw_gui = !self.should_draw_gui
    }
}
