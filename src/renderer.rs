use async_std::task::block_on;
use glam::{Quat, Vec3};
use std::{cell::RefCell, f32::consts, fs::File, io::Write, rc::Rc, time::Duration};
use wgpu::{util::DeviceExt, InstanceDescriptor, RenderPassDepthStencilAttachment};

use crate::{
    bind_group_layout_descriptors,
    buffer_content::BufferContent,
    camera_controller::CameraController,
    color,
    gbuffer::GBuffer,
    gui::{Gui, GuiParams},
    instance::{self, Instance},
    light_controller::LightController,
    model::{Material, Mesh, Model},
    primitive_shapes,
    render_pipeline::RenderPipeline,
    resources,
    shadow_pipeline::Shadow,
    skybox_pipeline,
    texture::{self, Texture},
    vertex,
};

pub const MAX_LIGHTS: usize = 10;
const NUM_INSTANCES_PER_ROW: u32 = 10;

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

    render_pipeline: RenderPipeline,
    light_render_pipeline: RenderPipeline,
    obj_model: Model,
    depth_texture: texture::Texture,

    skybox: skybox_pipeline::Skybox,

    instances: Vec<Instance>,
    instance_buffer: wgpu::Buffer,

    frame_content_copy_dest: OutputBuffer,

    shadow: Shadow,
    gbuffer: GBuffer,

    should_capture_frame_content: bool,
    should_draw_gui: bool,

    square: Mesh,
    square_instance_buffer: wgpu::Buffer,

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

        let tree_texture_raw = include_bytes!("../assets/happy-tree.png");

        let tree_texture =
            texture::Texture::from_bytes(&device, &queue, tree_texture_raw, "treeTexture").unwrap();

        let depth_texture = texture::Texture::create_depth_texture(
            &device,
            config.width,
            config.height,
            "depth_texture",
        );

        let shader_desc = wgpu::ShaderModuleDescriptor {
            label: Some("GBuffer processing shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/gbuffer_pass.wgsl").into()),
        };

        let shadow = crate::shadow_pipeline::Shadow::new(&device);

        let main_shader = device.create_shader_module(shader_desc);

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Main Render Pipeline Layout"),
                bind_group_layouts: &[
                    &device.create_bind_group_layout(&bind_group_layout_descriptors::LIGHT),
                    &device.create_bind_group_layout(&bind_group_layout_descriptors::CAMERA),
                    &device.create_bind_group_layout(&bind_group_layout_descriptors::GBUFFER),
                    &device.create_bind_group_layout(&bind_group_layout_descriptors::DEPTH_TEXTURE),
                ],
                push_constant_ranges: &[],
            });

        let render_pipeline = RenderPipeline::new(
            Some("Main render pipeline"),
            &device,
            &render_pipeline_layout,
            config.format,
            Some(texture::Texture::DEPTH_FORMAT),
            &[],
            &main_shader,
        );

        let light_render_pipeline = {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Light Pipeline Layout"),
                bind_group_layouts: &[
                    &device.create_bind_group_layout(&bind_group_layout_descriptors::LIGHT),
                    &device.create_bind_group_layout(&bind_group_layout_descriptors::CAMERA),
                ],
                push_constant_ranges: &[],
            });
            let light_shader_desc = wgpu::ShaderModuleDescriptor {
                label: Some("Light Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/light.wgsl").into()),
            };
            let light_shader = device.create_shader_module(light_shader_desc);
            RenderPipeline::new(
                Some("Light render pipeline"),
                &device,
                &layout,
                config.format,
                Some(texture::Texture::DEPTH_FORMAT),
                &[vertex::VertexRaw::buffer_layout()],
                &light_shader,
            )
        };

        const SPACE_BETWEEN: f32 = 4.0;
        const SCALE: Vec3 = Vec3::new(1.0, 1.0, 1.0);
        let instances = (0..NUM_INSTANCES_PER_ROW)
            .flat_map(|z| {
                (0..NUM_INSTANCES_PER_ROW).map(move |x| {
                    let x = SPACE_BETWEEN * (x as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0);
                    let z = SPACE_BETWEEN * (z as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0);

                    let position = Vec3 { x, y: 0.0, z };

                    let rotation = if position == Vec3::ZERO {
                        Quat::from_axis_angle(Vec3::Z, 0.0)
                    } else {
                        Quat::from_axis_angle(position.normalize(), consts::FRAC_PI_4)
                    };

                    Instance {
                        position,
                        rotation,
                        scale: SCALE,
                    }
                })
            })
            .collect::<Vec<_>>();

        let instance_data = instances
            .iter()
            .map(instance::Instance::to_raw)
            .collect::<Vec<_>>();
        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instance Buffer"),
            contents: bytemuck::cast_slice(&instance_data),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let obj_model = resources::load_model("cube.obj", &device, &queue)
            .await
            .unwrap();

        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &device
                .create_bind_group_layout(&bind_group_layout_descriptors::DIFFUSE_TEXTURE),
            entries: &[
                tree_texture.get_texture_bind_group_entry(0),
                tree_texture.get_sampler_bind_group_entry(1),
            ],
            label: Some("diffuse_bind_group"),
        });

        let square_material = Some(Rc::new(Material {
            name: "Tree texture material".into(),
            diffuse_texture: tree_texture,
            bind_group: texture_bind_group,
        }));

        let square = primitive_shapes::square(&device, square_material);

        let square_instances = vec![Instance {
            position: Vec3::new(0.0, -10.0, 0.0),
            rotation: Quat::IDENTITY,
            scale: 100.0_f32
                * Vec3 {
                    x: 1.0_f32,
                    y: 1.0,
                    z: 1.0,
                },
        }];

        let square_instance_raw = square_instances
            .iter()
            .map(|instance| instance.to_raw())
            .collect::<Vec<_>>();
        let square_instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Square Instance Buffer"),
            contents: bytemuck::cast_slice(&square_instance_raw),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let skybox = skybox_pipeline::Skybox::new(&device, &queue, config.format);

        let gbuffer = GBuffer::new(&device, config.width, config.height);

        Renderer {
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
            light_render_pipeline,
            obj_model,
            depth_texture,
            instances,
            instance_buffer,
            frame_content_copy_dest: output_buffer,
            should_capture_frame_content: false,
            should_draw_gui: true,
            square,
            square_instance_buffer,
            gui,
            gui_params,
            shadow,
            skybox,
            gbuffer,
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
        self.gbuffer
            .resize(&self.device, new_size.width, new_size.height);
        self.frame_content_copy_dest =
            OutputBuffer::new(&self.device, new_size.width, new_size.height);
    }

    pub fn render(
        &mut self,
        window: &winit::window::Window,
        camera_controller: &CameraController,
        light_controller: &LightController,
        delta: Duration,
    ) -> Result<(), wgpu::SurfaceError> {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        self.shadow.render(
            &mut encoder,
            &self.obj_model,
            &light_controller.bind_group,
            self.instances.len(),
            &self.instance_buffer,
        );

        self.gbuffer.render_model(
            &mut encoder,
            &self.obj_model,
            &camera_controller.bind_group,
            self.instances.len(),
            &self.instance_buffer,
        );

        self.gbuffer.render_mesh(
            &mut encoder,
            &self.square,
            &camera_controller.bind_group,
            1,
            &self.square_instance_buffer,
        );

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
                    view: &self.gbuffer.textures.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: false,
                    }),
                    stencil_ops: None,
                }),
            });

            // render_pass.set_vertex_buffer(1, self.square_instance_buffer.slice(..));
            // self.square.draw_instanced(&mut render_pass, 0..1);

            {
                render_pass.push_debug_group("Cubes rendering from GBuffer");

                render_pass.set_pipeline(&self.render_pipeline.pipeline);

                render_pass.set_bind_group(1, &camera_controller.bind_group, &[]);
                render_pass.set_bind_group(0, &light_controller.bind_group, &[]);
                render_pass.set_bind_group(2, &self.gbuffer.bind_group, &[]);
                render_pass.set_bind_group(3, &self.shadow.bind_group, &[]);

                render_pass.draw(0..3, 0..1);

                render_pass.pop_debug_group();
            }

            {
                render_pass.push_debug_group("Skybox rendering");

                self.skybox.render(&mut render_pass, camera_controller);

                render_pass.pop_debug_group();
            }
        }

        // {
        //     let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        //         label: Some("Forward Render Pass"),
        //         color_attachments: &[Some(wgpu::RenderPassColorAttachment {
        //             view: &view,
        //             resolve_target: None,
        //             ops: wgpu::Operations {
        //                 load: wgpu::LoadOp::Clear(color::f32_array_rgba_to_wgpu_color(
        //                     self.gui_params.clear_color,
        //                 )),
        //                 store: true,
        //             },
        //         })],
        //         depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
        //             view: &self.gbuffer.depth_texture.view,
        //             depth_ops: Some(wgpu::Operations {
        //                 load: wgpu::LoadOp::Load,
        //                 store: true,
        //             }),
        //             stencil_ops: None,
        //         }),
        //     });

        //     render_pass.push_debug_group("Light rendering");
        //     use crate::model::DrawLight;
        //     render_pass.set_pipeline(&self.light_render_pipeline.pipeline);
        //     render_pass.draw_light_model(
        //         &self.obj_model,
        //         &camera_controller.bind_group,
        //         &light_controller.bind_group,
        //     );
        //     render_pass.pop_debug_group();
        // }

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
